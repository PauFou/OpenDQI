//! `opendqi sftr ...` subcommands.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use clap::Subcommand;
use opendqi_core::dq::compute_sftr_book_reconcile_issues;
use opendqi_core::dq::{
    default_missing_collateral_checks, default_sftr_checks, default_sftr_lifecycle_checks,
    default_sftr_pre_submission_checks, default_sftr_reconciliation_checks,
    default_sftr_tr_activity_checks, default_sftr_tr_state_checks,
    default_sftr_tr_state_lifecycle_checks, run_all_sftr_lifecycle, run_all_sftr_pre_submission,
    CheckContext,
};
use opendqi_core::{
    compute_sftr_dqi_pack, stream_checks_into, DqDimension, DqIssue, MappingPresence, Regime,
    Severity, SftrDqiInputs, SftrRecord, SftrTrStateRecord, SortedIssueSink, Thresholds,
    TrActivitySummary, STREAM_SPILL_MAX_ISSUES,
};
use opendqi_io::{
    discover_emir_inputs, has_extension, read_sftr_csv, read_sftr_parquet, write_sftr_parquet,
    CsvMapping,
};
use opendqi_report::{
    write_evidence_csv, write_indicators_csv, write_issues_csv, write_issues_csv_from_iter,
    write_report_html, write_report_html_with_dqi, write_summary_json, TopIssues,
};
use opendqi_xml::{
    check_wellformedness, read_sftr_margin_state_xml, read_sftr_missing_collateral_xml,
    read_sftr_reconciliation_xml, read_sftr_tr_state_xml, read_sftr_xml, ExternalXmllintValidator,
    XsdValidator, XsdViolation,
};
use tracing::{info, warn};

const CHECK_XSD_VIOLATION: &str = "SFTR.FMT.XSD_VIOLATION";
const CHECK_XSD_TOOL_ERROR: &str = "SFTR.FMT.XSD_TOOL_ERROR";

#[derive(Subcommand)]
pub enum SftrAction {
    /// Run the full DQ scan over an SFTR XML or CSV input (file or
    /// directory).
    Scan {
        /// Path to an XML/CSV file or a directory containing such files.
        input: PathBuf,
        /// Directory where reports are written. Created if absent.
        #[arg(long)]
        out: PathBuf,
        /// Path to the YAML mapping describing CSV columns. Required
        /// when the input set contains at least one CSV file.
        #[arg(long)]
        mapping: Option<PathBuf>,
        /// Optional YAML thresholds configuration.
        #[arg(long)]
        config: Option<PathBuf>,
        /// Optional XSD schema. When set, every XML input is also
        /// validated against the schema and violations are added to
        /// the report (one `SFTR.FMT.XSD_VIOLATION` per error line).
        #[arg(long)]
        xsd: Option<PathBuf>,
        /// Optional SQLite history-store path. When set, scanned
        /// records are persisted and cross-batch lifecycle checks
        /// (MODI-without-NEWT, ETRM-without-NEWT, duplicate NEWT)
        /// run against the accumulated history.
        #[arg(long, value_name = "PATH")]
        store: Option<PathBuf>,
        /// Optional path to a `rejection_profile.yml` exported by
        /// `opendqi feedback analytics`. When set, the scan runs the
        /// pre-submission `SFTR.PSC.*` family that flags records
        /// likely to be rejected by the TR based on historical
        /// patterns. See `docs/pre-submission-checks.md`.
        #[arg(long, value_name = "PATH")]
        rejection_profile: Option<PathBuf>,
        /// Optional SMTP configuration YAML. When set, the HTML
        /// report + `summary.json` + `issues.csv` are emailed to the
        /// recipients listed in the config after the scan. See
        /// `docs/email-notifications.md`.
        #[arg(long, value_name = "PATH")]
        email_config: Option<PathBuf>,
    },
    /// Validate XML files: well-formedness check + XSD validation.
    /// Exits non-zero when at least one issue is found.
    Validate {
        /// Path to an XML file or a directory of XML files.
        input: PathBuf,
        /// Required path to an XSD schema file.
        #[arg(long)]
        xsd: PathBuf,
    },
    /// Ingest a TR pairing / matching report and produce
    /// `SFTR.REC.*` issues for UNPAIRED / UNRECONCILED trades and
    /// field-level mismatches.
    ///
    /// Naming caveat: this command reads a synthetic structure
    /// documented as `auth.083` v1; the official ESMA message may
    /// carry different semantics. See `docs/auth-messages.md`.
    Reconcile {
        /// Path to the `auth.083` XML file received from the TR.
        input: PathBuf,
        /// Required path to the SQLite history store.
        #[arg(long)]
        store: PathBuf,
        /// Directory where reports are written.
        #[arg(long)]
        out: PathBuf,
        /// Optional SMTP configuration YAML — emails the SFTR
        /// reconciliation report. See `docs/email-notifications.md`.
        #[arg(long, value_name = "PATH")]
        email_config: Option<PathBuf>,
    },
    /// Ingest an SFTR Trade State Report (ISO 20022 `auth.079`) and
    /// produce `SFTR.TST.*` issues over the TR's snapshot:
    /// outstanding summary, stale collateral valuation, missing
    /// collateral on outstanding SFT, active past maturity, duplicate
    /// active UTI, haircut out of range. Outputs
    /// `summary.json`, `tr_state_issues.csv`, `tr_state_report.html`.
    TrStateScan {
        /// Path to the `auth.079` XML file received from the TR.
        input: PathBuf,
        /// Optional path to the SQLite history store (loads prior SFT
        /// records for the UTIs in the TSR — symmetric with EMIR).
        #[arg(long)]
        store: Option<PathBuf>,
        /// Directory where reports are written.
        #[arg(long)]
        out: PathBuf,
        /// Optional SMTP configuration YAML — emails the SFTR TSR
        /// report. See `docs/email-notifications.md`.
        #[arg(long, value_name = "PATH")]
        email_config: Option<PathBuf>,
    },
    /// Ingest an SFTR Trade Activity Report (TR replay of
    /// `auth.052`) and produce `SFTR.TRA.*` issues: repeated
    /// corrections, ETRM / MODI spikes, duplicate NEWT in batch.
    /// With `--tsr <auth.079>` also runs TAR↔TSR coherence checks.
    /// Outputs `summary.json`, `tr_activity_issues.csv`,
    /// `tr_activity_report.html`.
    TrActivityScan {
        /// Path to the `auth.052` XML file (firm-submitted or TR
        /// replay) — directory accepted.
        input: PathBuf,
        /// Optional path to the SQLite history store.
        #[arg(long)]
        store: Option<PathBuf>,
        /// Optional companion SFTR TSR (`auth.079`). When set,
        /// triggers the TAR↔TSR coherence check.
        #[arg(long)]
        tsr: Option<PathBuf>,
        /// Directory where reports are written.
        #[arg(long)]
        out: PathBuf,
        /// Optional SMTP configuration YAML — emails the SFTR TAR
        /// report. See `docs/email-notifications.md`.
        #[arg(long, value_name = "PATH")]
        email_config: Option<PathBuf>,
    },
    /// Consolidated SFTR TR audit. Ingests a TAR (`auth.052`) and a
    /// TSR (`auth.079`) together, runs every layer's checks plus the
    /// 2 cross-layer coherence checks (`SFTR.AUD.*`), and writes a
    /// single `tr_audit_report.html` consolidating both layers.
    /// (SFTR has no rejection-feedback message — there is no feedback
    /// layer; use `opendqi emir tr-audit` for the EMIR equivalent
    /// which does include feedback.)
    TrAudit {
        /// Path to the TAR `auth.052` XML file (or directory).
        #[arg(long)]
        tar: PathBuf,
        /// Path to the TSR `auth.079` XML file.
        #[arg(long)]
        tsr: PathBuf,
        /// Optional path to the SQLite history store.
        #[arg(long)]
        store: Option<PathBuf>,
        /// Directory where reports are written.
        #[arg(long)]
        out: PathBuf,
        /// Optional SMTP configuration YAML — emails the consolidated
        /// SFTR TR audit report. See `docs/email-notifications.md`.
        #[arg(long, value_name = "PATH")]
        email_config: Option<PathBuf>,
    },
    /// Reconcile a firm's internal SFT book export (CSV) against a
    /// TR SFTR Trade State Report (`auth.079`). Produces
    /// `SFTR.BREC.*` issues for missing UTIs (in either direction)
    /// and field mismatches on loan / collateral / maturity / status.
    /// Outputs `summary.json`, `book_vs_tsr_issues.csv`,
    /// `book_vs_tsr_report.html`.
    BookReconcile {
        /// Path to the internal SFT book export (CSV).
        #[arg(long)]
        book: PathBuf,
        /// Path to the TR `auth.079` SFTR TSR XML.
        #[arg(long)]
        tsr: PathBuf,
        /// Path to the YAML mapping describing book CSV columns.
        #[arg(long)]
        mapping: PathBuf,
        /// Directory where reports are written.
        #[arg(long)]
        out: PathBuf,
        /// Optional SMTP configuration YAML — emails the SFTR
        /// book-vs-TSR report. See `docs/email-notifications.md`.
        #[arg(long, value_name = "PATH")]
        email_config: Option<PathBuf>,
    },
    /// Ingest an SFTR Missing Collateral Request (`auth.083`, the TR
    /// asking the firm to provide the collateral missing for listed
    /// SFTs) and produce `SFTR.MCR.*` issues: one actionable request
    /// per `TxId`, plus a flag for requests with no UTI. With `--tsr`
    /// (or `--store`) the requested UTIs are cross-referenced against
    /// the firm's SFTR trade state to flag whether collateral is
    /// already present / still missing / the SFT is absent. Outputs
    /// `summary.json`, `missing_collateral_issues.csv`,
    /// `missing_collateral_report.html`.
    MissingCollateral {
        /// Path to the `auth.083` XML file.
        input: PathBuf,
        /// Optional companion SFTR TSR (`auth.079`). When set, the
        /// requested UTIs are cross-referenced against it. Takes
        /// precedence over `--store`.
        #[arg(long)]
        tsr: Option<PathBuf>,
        /// Optional SQLite history store. When set (and `--tsr` is
        /// not), the latest persisted SFTR trade state is used for
        /// the cross-reference.
        #[arg(long)]
        store: Option<PathBuf>,
        /// Directory where reports are written.
        #[arg(long)]
        out: PathBuf,
        /// Optional SMTP configuration YAML — emails the SFTR
        /// missing-collateral report. See `docs/email-notifications.md`.
        #[arg(long, value_name = "PATH")]
        email_config: Option<PathBuf>,
    },
    /// SFTR Data Quality Pack v1 (v0.16). Aggregates the
    /// SFTR TSR + TAR layers into 4 regulator-style indicators
    /// (numerator / denominator / rate / threshold / status).
    /// Granular SFTR checks are co-produced into `issues.csv`.
    /// See `docs/data-quality-pack.md`.
    ///
    /// All input flags are optional ; at least one is required.
    ///
    /// v0.17 ships **16 SFTR DQIs** total spanning 5 input
    /// layers — TSR (auth.079), TAR (auth.052), reconciliation
    /// (auth.080), missing-collateral (auth.083) and MSR
    /// (auth.085). Indicators whose source layer isn't
    /// provided self-report `not_applicable`.
    DataQualityPack {
        /// Path to the SFTR TSR `auth.079` XML file. Powers 6
        /// TSR-layer DQIs (loan/collateral missing/stale,
        /// haircut anomaly, LEI missing, under-collateralization).
        #[arg(long)]
        tsr: Option<PathBuf>,
        /// Path to the SFTR TAR `auth.052` XML file (or directory).
        /// Powers DQI_TIM_REPORTING_LATE_SFTR.
        #[arg(long)]
        tar: Option<PathBuf>,
        /// Path to the `auth.080` SFTR Reconciliation Status
        /// Advice. Powers 4 reconciliation DQIs (PAIRING_RATE,
        /// RECONCILIATION_RATE, UNPAIRED_TRADES_RATE,
        /// FIELD_MISMATCH_RATE).
        #[arg(long)]
        reconciliation: Option<PathBuf>,
        /// Path to the `auth.083` SFTR Missing-Collateral
        /// request. Powers DQI_MCR_OPEN_REQUESTS_SFTR (best
        /// paired with --tsr for portfolio cross-ref).
        #[arg(long)]
        missing_collateral: Option<PathBuf>,
        /// Path to the `auth.085` SFTR Margin Data Transaction
        /// State Report (MSR). Powers 4 T3 margin DQIs
        /// (MARGIN_POSTED/RECEIVED_MISSING, EXCESS_COLL_USE,
        /// MARGIN_STALE) AND 6 granular SFTR.T3.* per-record
        /// checks. CCP-cleared SFT portfolios only per the
        /// ESMA scope.
        #[arg(long)]
        msr: Option<PathBuf>,
        /// Optional YAML thresholds configuration (overrides
        /// the shipped DQI defaults via the `dqi:` block).
        #[arg(long)]
        config: Option<PathBuf>,
        /// Reference date for age-based indicators (stale
        /// loan value). Defaults to today (UTC). Format
        /// YYYY-MM-DD.
        #[arg(long)]
        as_of: Option<String>,
        /// Directory where outputs are written. Writes
        /// `report.html`, `summary.json`, `issues.csv`,
        /// `indicators.csv`, `evidence.csv`.
        #[arg(long)]
        out: PathBuf,
        /// Optional SMTP configuration YAML.
        #[arg(long, value_name = "PATH")]
        email_config: Option<PathBuf>,
    },
    /// Normalize SFTR XML/CSV input into a canonical Parquet file
    /// (Snappy-compressed). Schema is stable and analytics-friendly.
    /// See `docs/parquet-normalize.md`.
    Normalize {
        /// Path to an XML/CSV file or a directory of such files.
        input: PathBuf,
        /// Path to the YAML mapping describing CSV columns. Required
        /// when the input set contains at least one CSV file.
        #[arg(long)]
        mapping: Option<PathBuf>,
        /// Output Parquet file path.
        #[arg(long)]
        out: PathBuf,
    },
}

pub fn run(action: SftrAction) -> Result<ExitCode> {
    match action {
        SftrAction::Scan {
            input,
            out,
            mapping,
            config,
            xsd,
            store,
            rejection_profile,
            email_config,
        } => {
            run_scan(
                &input,
                &out,
                mapping.as_deref(),
                config.as_deref(),
                xsd.as_deref(),
                store.as_deref(),
                rejection_profile.as_deref(),
                email_config.as_deref(),
            )?;
            Ok(ExitCode::SUCCESS)
        }
        SftrAction::Validate { input, xsd } => run_validate(&input, &xsd),
        SftrAction::Reconcile {
            input,
            store,
            out,
            email_config,
        } => {
            run_reconcile(&input, &store, &out, email_config.as_deref())?;
            Ok(ExitCode::SUCCESS)
        }
        SftrAction::TrStateScan {
            input,
            store,
            out,
            email_config,
        } => {
            run_tr_state_scan(&input, store.as_deref(), &out, email_config.as_deref())?;
            Ok(ExitCode::SUCCESS)
        }
        SftrAction::TrActivityScan {
            input,
            store,
            tsr,
            out,
            email_config,
        } => {
            run_tr_activity_scan(
                &input,
                store.as_deref(),
                tsr.as_deref(),
                &out,
                email_config.as_deref(),
            )?;
            Ok(ExitCode::SUCCESS)
        }
        SftrAction::TrAudit {
            tar,
            tsr,
            store,
            out,
            email_config,
        } => {
            run_tr_audit(&tar, &tsr, store.as_deref(), &out, email_config.as_deref())?;
            Ok(ExitCode::SUCCESS)
        }
        SftrAction::BookReconcile {
            book,
            tsr,
            mapping,
            out,
            email_config,
        } => {
            run_book_reconcile(&book, &tsr, &mapping, &out, email_config.as_deref())?;
            Ok(ExitCode::SUCCESS)
        }
        SftrAction::MissingCollateral {
            input,
            tsr,
            store,
            out,
            email_config,
        } => {
            run_missing_collateral(
                &input,
                tsr.as_deref(),
                store.as_deref(),
                &out,
                email_config.as_deref(),
            )?;
            Ok(ExitCode::SUCCESS)
        }
        SftrAction::DataQualityPack {
            tsr,
            tar,
            reconciliation,
            missing_collateral,
            msr,
            config,
            as_of,
            out,
            email_config,
        } => {
            run_data_quality_pack(
                tsr.as_deref(),
                tar.as_deref(),
                reconciliation.as_deref(),
                missing_collateral.as_deref(),
                msr.as_deref(),
                config.as_deref(),
                as_of.as_deref(),
                &out,
                email_config.as_deref(),
            )?;
            Ok(ExitCode::SUCCESS)
        }
        SftrAction::Normalize {
            input,
            mapping,
            out,
        } => {
            run_normalize(&input, mapping.as_deref(), &out)?;
            Ok(ExitCode::SUCCESS)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn run_scan(
    input: &Path,
    out: &Path,
    mapping_path: Option<&Path>,
    config_path: Option<&Path>,
    xsd_path: Option<&Path>,
    store_path: Option<&Path>,
    rejection_profile_path: Option<&Path>,
    email_config_path: Option<&Path>,
) -> Result<()> {
    let started_at = Utc::now();

    let thresholds = match config_path {
        Some(p) => {
            let text = std::fs::read_to_string(p)
                .with_context(|| format!("reading thresholds config {}", p.display()))?;
            serde_yaml::from_str::<Thresholds>(&text)
                .with_context(|| format!("parsing thresholds config {}", p.display()))?
        }
        None => Thresholds::default(),
    };

    let inputs = discover_emir_inputs(input)?;
    if inputs.is_empty() {
        return Err(anyhow!("no inputs found at {}", input.display()));
    }
    info!(count = inputs.len(), "discovered inputs");

    let has_csv = inputs.iter().any(|p| has_extension(p, "csv"));
    let csv_mapping = match (has_csv, mapping_path) {
        (true, Some(mp)) => Some(
            CsvMapping::from_path(mp)
                .with_context(|| format!("loading CSV mapping {}", mp.display()))?,
        ),
        (true, None) => {
            return Err(anyhow!(
                "input set contains CSV files but --mapping was not provided"
            ));
        }
        (false, _) => None,
    };

    let validator = xsd_path.map(|p| ExternalXmllintValidator::new(p.to_path_buf()));
    crate::memtrace::mem_trace("post_discovery");
    // Sampler covers parse→report (discovery is sub-second / ~MiB; the
    // GB-scale peak we hunt is downstream). RAII: named binding so it
    // lives to fn end; Drop stops+joins+prints the true-peak line.
    let _peak_sampler = crate::memtrace::start_peak_sampler();

    let mut records: Vec<SftrRecord> = Vec::new();
    let mut format_issues: Vec<DqIssue> = Vec::new();
    let mut xsd_rows: Vec<XsdReportRow> = Vec::new();
    let mut sources: Vec<String> = Vec::with_capacity(inputs.len());

    for path in &inputs {
        sources.push(path.to_string_lossy().into_owned());
        if has_extension(path, "xml") {
            let mut outcome = read_sftr_xml(path)?;
            info!(
                file = %path.display(),
                records = outcome.records.len(),
                format_issues = outcome.issues.len(),
                "loaded SFTR XML",
            );
            records.append(&mut outcome.records);
            format_issues.append(&mut outcome.issues);

            if let Some(v) = validator.as_ref() {
                match v.validate(path) {
                    Ok(violations) => {
                        info!(
                            file = %path.display(),
                            violations = violations.len(),
                            "xsd validated"
                        );
                        for vio in &violations {
                            format_issues.push(xsd_violation_issue(path, vio));
                            xsd_rows.push(XsdReportRow::from_violation(path, vio));
                        }
                    }
                    Err(err) => {
                        warn!(file = %path.display(), error = %err, "xsd tool error");
                        format_issues.push(xsd_tool_error_issue(path, &err.message));
                    }
                }
            }
        } else if has_extension(path, "csv") {
            let mapping = csv_mapping
                .as_ref()
                .expect("csv_mapping is Some when at least one CSV is in the input set");
            let mut csv_records = read_sftr_csv(path, mapping)?;
            info!(
                file = %path.display(),
                records = csv_records.len(),
                "loaded SFTR CSV",
            );
            records.append(&mut csv_records);
        } else if has_extension(path, "parquet") {
            let mut pq_records = read_sftr_parquet(path)?;
            info!(
                file = %path.display(),
                records = pq_records.len(),
                "loaded SFTR Parquet",
            );
            records.append(&mut pq_records);
        } else {
            warn!(path = %path.display(), "unsupported file extension; supported: XML, CSV, Parquet");
        }
    }

    crate::memtrace::mem_trace("post_parse");

    let now = Utc::now();
    let ctx = CheckContext {
        thresholds,
        today: now.date_naive(),
        now,
    };

    let checks = default_sftr_checks();
    // Streaming pipeline (Milestone 0.26): every issue source is
    // pushed into one spill-capable sink instead of a union
    // `Vec<DqIssue>` — mirrors the M0.23 EMIR `run_scan`. Small
    // inputs never spill ⇒ output byte-identical.
    let sink = std::sync::Mutex::new(SortedIssueSink::new(
        &ctx.thresholds,
        STREAM_SPILL_MAX_ISSUES,
    ));
    stream_checks_into(&checks, &sink, |c| c.run(&records, &ctx));
    sink.lock()
        .expect("issue sink mutex")
        .push_batch(format_issues);
    crate::memtrace::mem_trace("post_checks");

    if let Some(store_path) = store_path {
        let mut store = opendqi_store::open_store(store_path)
            .with_context(|| format!("opening history store at {}", store_path.display()))?;
        let scan_id = store
            .persist_sftr_batch(inputs.len(), &records)
            .context("persisting SFTR batch to history store")?;
        let utis: Vec<&str> = records
            .iter()
            .filter_map(|r| r.uti.as_deref())
            .map(str::trim)
            .filter(|u| !u.is_empty())
            .collect();
        let prior = store
            .load_prior_sftr(&utis, scan_id)
            .context("loading prior SFTR records from history store")?;
        info!(prior_records = prior.len(), "loaded prior records");
        let lifecycle_issues =
            run_all_sftr_lifecycle(&default_sftr_lifecycle_checks(), &records, &prior, &ctx);
        info!(
            lifecycle_issues = lifecycle_issues.len(),
            "lifecycle checks run"
        );
        sink.lock()
            .expect("issue sink mutex")
            .push_batch(lifecycle_issues);
    }

    if let Some(profile_path) = rejection_profile_path {
        let text = std::fs::read_to_string(profile_path)
            .with_context(|| format!("reading rejection profile {}", profile_path.display()))?;
        let file: opendqi_core::RejectionProfileFile = serde_yaml::from_str(&text)
            .with_context(|| format!("parsing rejection profile {}", profile_path.display()))?;
        let profile = file.profile;
        info!(
            top_causes = profile.top_causes.len(),
            repeated_rejected_utis = profile.repeated_rejected_utis.len(),
            "loaded rejection profile"
        );
        let psc_issues = run_all_sftr_pre_submission(
            &default_sftr_pre_submission_checks(),
            &records,
            &profile,
            &ctx,
        );
        info!(
            psc_issues = psc_issues.len(),
            "SFTR pre-submission checks run"
        );
        sink.lock()
            .expect("issue sink mutex")
            .push_batch(psc_issues);
    }

    crate::memtrace::mem_trace("post_lifecycle_presub");

    let (summary, sorted) = sink.into_inner().expect("issue sink mutex").finish(
        Regime::Sftr,
        inputs.len() as u32,
        records.len() as u32,
        started_at,
        Utc::now(),
    );
    crate::memtrace::mem_trace("post_finalize");
    crate::memtrace::mem_trace("post_summary");

    std::fs::create_dir_all(out)
        .with_context(|| format!("creating output directory {}", out.display()))?;
    write_summary_json(&out.join("summary.json"), &summary)?;
    crate::memtrace::mem_trace("post_write_summary_json");
    let mut top = TopIssues::with_capacity(20);
    write_issues_csv_from_iter(
        &out.join("issues.csv"),
        sorted.inspect(|issue| top.offer(issue)),
    )?;
    crate::memtrace::mem_trace("post_write_issues_csv");
    let top = top.into_sorted();
    write_report_html(&out.join("report.html"), &summary, &top, &sources)?;
    crate::memtrace::mem_trace("post_write_report_html");
    if validator.is_some() {
        write_xsd_errors_csv(&out.join("xsd_errors.csv"), &xsd_rows)?;
    }
    crate::memtrace::mem_trace("post_report");

    if let Some(path) = email_config_path {
        let cfg = opendqi_report::SmtpConfig::from_yaml_file(path)?;
        let sent = opendqi_report::send_report_email(
            &cfg,
            &summary,
            &out.join("report.html"),
            &out.join("summary.json"),
            &out.join("issues.csv"),
        )?;
        if sent {
            info!(to = ?cfg.to, "scan report emailed");
        } else {
            info!("email config is disabled — skipped send");
        }
    }

    let critical = summary
        .issues_by_severity
        .get(&Severity::Critical)
        .copied()
        .unwrap_or(0);
    let high = summary
        .issues_by_severity
        .get(&Severity::High)
        .copied()
        .unwrap_or(0);
    println!(
        "Scanned {} SFTR records across {} file(s). {} issues ({} critical, {} high). Score: {:.1}/100.",
        summary.records_processed,
        summary.files_processed,
        summary.issues_total,
        critical,
        high,
        summary.quality_score
    );
    println!("Report: {}", out.join("report.html").display());
    Ok(())
}

fn run_missing_collateral(
    input: &Path,
    tsr_path: Option<&Path>,
    store_path: Option<&Path>,
    out: &Path,
    email_config_path: Option<&Path>,
) -> Result<()> {
    let started_at = Utc::now();
    let outcome = read_sftr_missing_collateral_xml(input)
        .with_context(|| format!("reading SFTR missing-collateral file {}", input.display()))?;
    info!(
        file = %input.display(),
        records = outcome.records.len(),
        format_issues = outcome.issues.len(),
        "loaded SFTR auth.083 Missing Collateral Request XML",
    );

    // Optional companion TSR for the cross-reference checks: `--tsr`
    // wins; else the latest persisted SFTR trade state for the
    // requested UTIs (read-only — auth.083 persists nothing).
    let tsr_records: Option<Vec<SftrTrStateRecord>> = match (tsr_path, store_path) {
        (Some(tsr), _) => {
            let o = read_sftr_tr_state_xml(tsr)
                .with_context(|| format!("reading companion SFTR TSR {}", tsr.display()))?;
            info!(
                tsr = %tsr.display(),
                tsr_records = o.records.len(),
                "loaded companion SFTR TSR for cross-reference"
            );
            Some(o.records)
        }
        (None, Some(store)) => {
            let store = opendqi_store::open_store(store)
                .with_context(|| format!("opening history store at {}", store.display()))?;
            let utis: Vec<&str> = outcome
                .records
                .iter()
                .filter_map(|r| r.uti.as_deref())
                .map(str::trim)
                .filter(|u| !u.is_empty())
                .collect();
            let prior = store
                .load_prior_sftr_tr_state(&utis, i64::MAX)
                .context("loading prior SFTR trade state from history store")?;
            info!(
                prior_tsr_records = prior.len(),
                "loaded prior SFTR trade state from store for cross-reference"
            );
            Some(prior)
        }
        (None, None) => None,
    };

    let now = Utc::now();
    let ctx = CheckContext {
        thresholds: Thresholds::default(),
        today: now.date_naive(),
        now,
    };
    // Streaming pipeline (Milestone 0.26): format + missing-collateral
    // issues fold into one spill-capable sink; `records_processed` =
    // parsed auth.083 record count (exactly as the former
    // `build_feedback_summary`). Byte-identical for non-spilling inputs.
    let inputs = [input.to_path_buf()];
    let sources = vec![input.to_string_lossy().into_owned()];
    let sink = std::sync::Mutex::new(SortedIssueSink::new(
        &ctx.thresholds,
        STREAM_SPILL_MAX_ISSUES,
    ));
    sink.lock()
        .expect("issue sink mutex")
        .push_batch(outcome.issues);
    stream_checks_into(&default_missing_collateral_checks(), &sink, |c| {
        c.run(&outcome.records, tsr_records.as_deref(), &ctx)
    });
    let (summary, sorted) = sink.into_inner().expect("issue sink mutex").finish(
        Regime::Sftr,
        inputs.len() as u32,
        outcome.records.len() as u32,
        started_at,
        Utc::now(),
    );

    std::fs::create_dir_all(out)
        .with_context(|| format!("creating output directory {}", out.display()))?;
    write_summary_json(&out.join("summary.json"), &summary)?;
    let mut top = TopIssues::with_capacity(20);
    write_issues_csv_from_iter(
        &out.join("missing_collateral_issues.csv"),
        sorted.inspect(|issue| top.offer(issue)),
    )?;
    let top = top.into_sorted();
    write_report_html(
        &out.join("missing_collateral_report.html"),
        &summary,
        &top,
        &sources,
    )?;

    if let Some(path) = email_config_path {
        let cfg = opendqi_report::SmtpConfig::from_yaml_file(path)?;
        let sent = opendqi_report::send_report_email(
            &cfg,
            &summary,
            &out.join("missing_collateral_report.html"),
            &out.join("summary.json"),
            &out.join("missing_collateral_issues.csv"),
        )?;
        if sent {
            info!(to = ?cfg.to, "SFTR missing-collateral report emailed");
        } else {
            info!("email config is disabled — skipped send");
        }
    }

    let critical = summary
        .issues_by_severity
        .get(&Severity::Critical)
        .copied()
        .unwrap_or(0);
    let high = summary
        .issues_by_severity
        .get(&Severity::High)
        .copied()
        .unwrap_or(0);
    println!(
        "Ingested {} SFTR missing-collateral request(s). {} issues ({} critical, {} high). Score: {:.1}/100.",
        summary.records_processed,
        summary.issues_total,
        critical,
        high,
        summary.quality_score
    );
    println!(
        "Report: {}",
        out.join("missing_collateral_report.html").display()
    );
    Ok(())
}

fn run_normalize(input: &Path, mapping_path: Option<&Path>, out: &Path) -> Result<()> {
    let inputs = discover_emir_inputs(input)?;
    if inputs.is_empty() {
        return Err(anyhow!("no inputs found at {}", input.display()));
    }
    info!(count = inputs.len(), "discovered inputs");

    let has_csv = inputs.iter().any(|p| has_extension(p, "csv"));
    let csv_mapping = match (has_csv, mapping_path) {
        (true, Some(mp)) => Some(
            CsvMapping::from_path(mp)
                .with_context(|| format!("loading CSV mapping {}", mp.display()))?,
        ),
        (true, None) => {
            return Err(anyhow!(
                "input set contains CSV files but --mapping was not provided"
            ));
        }
        (false, _) => None,
    };

    let mut records: Vec<SftrRecord> = Vec::new();
    for path in &inputs {
        if has_extension(path, "xml") {
            let mut outcome = read_sftr_xml(path)?;
            info!(
                file = %path.display(),
                records = outcome.records.len(),
                "loaded SFTR XML for normalization",
            );
            records.append(&mut outcome.records);
        } else if has_extension(path, "csv") {
            let mapping = csv_mapping
                .as_ref()
                .expect("csv_mapping is Some when at least one CSV is in the input set");
            let mut csv_records = read_sftr_csv(path, mapping)?;
            info!(
                file = %path.display(),
                records = csv_records.len(),
                "loaded SFTR CSV for normalization",
            );
            records.append(&mut csv_records);
        } else {
            warn!(path = %path.display(), "unsupported file extension; only XML and CSV are normalized");
        }
    }

    write_sftr_parquet(out, &records)
        .with_context(|| format!("writing Parquet to {}", out.display()))?;

    let bytes = std::fs::metadata(out).map(|m| m.len()).unwrap_or(0);
    println!(
        "Normalized {} SFTR record(s) into {} ({} bytes, Snappy-compressed Parquet).",
        records.len(),
        out.display(),
        bytes
    );
    Ok(())
}

fn run_validate(input: &Path, xsd_path: &Path) -> Result<ExitCode> {
    let inputs = discover_emir_inputs(input)?;
    if inputs.is_empty() {
        return Err(anyhow!("no XML inputs found at {}", input.display()));
    }

    let xml_inputs: Vec<&PathBuf> = inputs.iter().filter(|p| has_extension(p, "xml")).collect();
    if xml_inputs.is_empty() {
        return Err(anyhow!(
            "opendqi sftr validate only supports XML files; pass an XML file or a directory containing XML files"
        ));
    }

    let validator = ExternalXmllintValidator::new(xsd_path.to_path_buf());
    let mut wf_errors = 0u32;
    let mut violations_total = 0u32;
    let mut tool_errors = 0u32;

    for path in &xml_inputs {
        match check_wellformedness(path) {
            Ok(()) => {}
            Err(err) => {
                wf_errors += 1;
                eprintln!("{}: not well-formed: {}", path.display(), err.message);
                continue;
            }
        }
        match validator.validate(path) {
            Ok(violations) => {
                violations_total += violations.len() as u32;
                for v in &violations {
                    let line = v.line.map(|n| format!("{n}")).unwrap_or_else(|| "?".into());
                    eprintln!("{}:{}: {}", path.display(), line, v.message);
                }
            }
            Err(err) => {
                tool_errors += 1;
                eprintln!("{}: xsd tool error: {}", path.display(), err.message);
            }
        }
    }

    println!(
        "Validated {} XML file(s). {} well-formedness errors, {} schema violations, {} tool errors.",
        xml_inputs.len(),
        wf_errors,
        violations_total,
        tool_errors,
    );

    let ok = wf_errors == 0 && violations_total == 0 && tool_errors == 0;
    Ok(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

fn run_reconcile(
    input: &Path,
    store_path: &Path,
    out: &Path,
    email_config_path: Option<&Path>,
) -> Result<()> {
    let started_at = Utc::now();
    let outcome = read_sftr_reconciliation_xml(input)
        .with_context(|| format!("reading reconciliation file {}", input.display()))?;
    info!(
        file = %input.display(),
        records = outcome.records.len(),
        format_issues = outcome.issues.len(),
        "loaded SFTR reconciliation XML",
    );

    let mut store = opendqi_store::open_store(store_path)
        .with_context(|| format!("opening history store at {}", store_path.display()))?;

    let persisted = store
        .persist_reconciliation_batch(&outcome.records)
        .context("persisting SFTR reconciliation batch to history store")?;
    info!(persisted, "reconciliation rows persisted to store");

    let utis: Vec<&str> = outcome
        .records
        .iter()
        .filter_map(|r| r.uti.as_deref())
        .map(str::trim)
        .filter(|u| !u.is_empty())
        .collect();
    let prior = store
        .load_prior_sftr(&utis, i64::MAX)
        .context("loading prior SFTR records from history store")?;
    info!(prior_records = prior.len(), "loaded prior records");

    let now = Utc::now();
    let ctx = CheckContext {
        thresholds: Thresholds::default(),
        today: now.date_naive(),
        now,
    };
    // Streaming pipeline (Milestone 0.26): format + reconciliation
    // issues fold into one spill-capable sink; `records_processed` =
    // parsed reconciliation record count (exactly as the former
    // `build_feedback_summary`). Byte-identical for non-spilling inputs.
    let inputs = [input.to_path_buf()];
    let sources = vec![input.to_string_lossy().into_owned()];
    let sink = std::sync::Mutex::new(SortedIssueSink::new(
        &ctx.thresholds,
        STREAM_SPILL_MAX_ISSUES,
    ));
    sink.lock()
        .expect("issue sink mutex")
        .push_batch(outcome.issues);
    stream_checks_into(&default_sftr_reconciliation_checks(), &sink, |c| {
        c.run(&outcome.records, &prior, &ctx)
    });
    let (summary, sorted) = sink.into_inner().expect("issue sink mutex").finish(
        Regime::Sftr,
        inputs.len() as u32,
        outcome.records.len() as u32,
        started_at,
        Utc::now(),
    );

    std::fs::create_dir_all(out)
        .with_context(|| format!("creating output directory {}", out.display()))?;
    write_summary_json(&out.join("summary.json"), &summary)?;
    let mut top = TopIssues::with_capacity(20);
    write_issues_csv_from_iter(
        &out.join("issues.csv"),
        sorted.inspect(|issue| top.offer(issue)),
    )?;
    let top = top.into_sorted();
    write_report_html(&out.join("report.html"), &summary, &top, &sources)?;

    if let Some(path) = email_config_path {
        let cfg = opendqi_report::SmtpConfig::from_yaml_file(path)?;
        let sent = opendqi_report::send_report_email(
            &cfg,
            &summary,
            &out.join("report.html"),
            &out.join("summary.json"),
            &out.join("issues.csv"),
        )?;
        if sent {
            info!(to = ?cfg.to, "SFTR reconciliation report emailed");
        } else {
            info!("email config is disabled — skipped send");
        }
    }

    let critical = summary
        .issues_by_severity
        .get(&Severity::Critical)
        .copied()
        .unwrap_or(0);
    let high = summary
        .issues_by_severity
        .get(&Severity::High)
        .copied()
        .unwrap_or(0);
    println!(
        "Ingested {} SFTR reconciliation record(s). {} issues ({} critical, {} high). Score: {:.1}/100.",
        summary.records_processed,
        summary.issues_total,
        critical,
        high,
        summary.quality_score
    );
    println!("Report: {}", out.join("report.html").display());
    Ok(())
}

fn run_tr_state_scan(
    input: &Path,
    store_path: Option<&Path>,
    out: &Path,
    email_config_path: Option<&Path>,
) -> Result<()> {
    let started_at = Utc::now();
    let outcome = read_sftr_tr_state_xml(input)
        .with_context(|| format!("reading SFTR TSR file {}", input.display()))?;
    info!(
        file = %input.display(),
        records = outcome.records.len(),
        format_issues = outcome.issues.len(),
        "loaded SFTR TSR XML",
    );

    let (prior, prior_tsr): (Vec<SftrRecord>, Vec<SftrTrStateRecord>) =
        if let Some(store_path) = store_path {
            let mut store = opendqi_store::open_store(store_path)
                .with_context(|| format!("opening history store at {}", store_path.display()))?;
            let scan_id = store
                .persist_sftr_tr_state_batch(1, &outcome.records)
                .context("persisting SFTR TSR batch to history store")?;
            let utis: Vec<&str> = outcome
                .records
                .iter()
                .filter_map(|r| r.uti.as_deref())
                .map(str::trim)
                .filter(|u| !u.is_empty())
                .collect();
            let prior = store
                .load_prior_sftr(&utis, i64::MAX)
                .context("loading prior SFTR records from history store")?;
            let prior_tsr = store
                .load_latest_prior_sftr_tr_state(scan_id)
                .context("loading prior SFTR TSR snapshot from history store")?;
            info!(
                prior_records = prior.len(),
                prior_tsr_records = prior_tsr.len(),
                "loaded prior records"
            );
            (prior, prior_tsr)
        } else {
            (Vec::new(), Vec::new())
        };

    let now = Utc::now();
    let ctx = CheckContext {
        thresholds: Thresholds::default(),
        today: now.date_naive(),
        now,
    };
    // Streaming pipeline (Milestone 0.26): SFTR TSR + conditional
    // lifecycle fold into one spill-capable sink; `records_processed`
    // = parsed TSR record count (exactly as the former
    // `build_feedback_summary`). Byte-identical for non-spilling inputs.
    let inputs = [input.to_path_buf()];
    let sources = vec![input.to_string_lossy().into_owned()];
    let sink = std::sync::Mutex::new(SortedIssueSink::new(
        &ctx.thresholds,
        STREAM_SPILL_MAX_ISSUES,
    ));
    sink.lock()
        .expect("issue sink mutex")
        .push_batch(outcome.issues);
    stream_checks_into(&default_sftr_tr_state_checks(), &sink, |c| {
        c.run(&outcome.records, &prior, &ctx)
    });
    if !prior_tsr.is_empty() {
        stream_checks_into(&default_sftr_tr_state_lifecycle_checks(), &sink, |c| {
            c.run(&outcome.records, &prior_tsr, &ctx)
        });
    }
    let (summary, sorted) = sink.into_inner().expect("issue sink mutex").finish(
        Regime::Sftr,
        inputs.len() as u32,
        outcome.records.len() as u32,
        started_at,
        Utc::now(),
    );

    std::fs::create_dir_all(out)
        .with_context(|| format!("creating output directory {}", out.display()))?;
    write_summary_json(&out.join("summary.json"), &summary)?;
    let mut top = TopIssues::with_capacity(20);
    write_issues_csv_from_iter(
        &out.join("tr_state_issues.csv"),
        sorted.inspect(|issue| top.offer(issue)),
    )?;
    let top = top.into_sorted();
    write_report_html(&out.join("tr_state_report.html"), &summary, &top, &sources)?;

    if let Some(path) = email_config_path {
        let cfg = opendqi_report::SmtpConfig::from_yaml_file(path)?;
        let sent = opendqi_report::send_report_email(
            &cfg,
            &summary,
            &out.join("tr_state_report.html"),
            &out.join("summary.json"),
            &out.join("tr_state_issues.csv"),
        )?;
        if sent {
            info!(to = ?cfg.to, "SFTR TSR report emailed");
        } else {
            info!("email config is disabled — skipped send");
        }
    }

    let critical = summary
        .issues_by_severity
        .get(&Severity::Critical)
        .copied()
        .unwrap_or(0);
    let high = summary
        .issues_by_severity
        .get(&Severity::High)
        .copied()
        .unwrap_or(0);
    println!(
        "Scanned {} SFTR TSR record(s). {} issues ({} critical, {} high). Score: {:.1}/100.",
        summary.records_processed, summary.issues_total, critical, high, summary.quality_score
    );
    println!("Report: {}", out.join("tr_state_report.html").display());
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_tr_activity_scan(
    input: &Path,
    store_path: Option<&Path>,
    tsr_path: Option<&Path>,
    out: &Path,
    email_config_path: Option<&Path>,
) -> Result<()> {
    let started_at = Utc::now();
    let inputs = discover_emir_inputs(input)?;
    if inputs.is_empty() {
        return Err(anyhow!("no XML inputs found at {}", input.display()));
    }
    info!(count = inputs.len(), "discovered SFTR TAR inputs");

    let mut records: Vec<SftrRecord> = Vec::new();
    let mut format_issues: Vec<DqIssue> = Vec::new();
    let mut sources: Vec<String> = Vec::with_capacity(inputs.len());

    for path in &inputs {
        sources.push(path.to_string_lossy().into_owned());
        if has_extension(path, "xml") {
            let mut outcome = read_sftr_xml(path)?;
            info!(
                file = %path.display(),
                records = outcome.records.len(),
                "loaded SFTR TAR XML",
            );
            records.append(&mut outcome.records);
            format_issues.append(&mut outcome.issues);
        } else {
            warn!(path = %path.display(), "unsupported file extension; only XML supported by sftr tr-activity-scan");
        }
    }

    let prior: Vec<SftrRecord> = if let Some(store_path) = store_path {
        let store = opendqi_store::open_store(store_path)
            .with_context(|| format!("opening history store at {}", store_path.display()))?;
        let utis: Vec<&str> = records
            .iter()
            .filter_map(|r| r.uti.as_deref())
            .map(str::trim)
            .filter(|u| !u.is_empty())
            .collect();
        let prior = store
            .load_prior_sftr(&utis, i64::MAX)
            .context("loading prior SFTR records from history store")?;
        info!(prior_records = prior.len(), "loaded prior records");
        prior
    } else {
        Vec::new()
    };

    let tsr_records: Option<Vec<SftrTrStateRecord>> = if let Some(tsr_path) = tsr_path {
        let outcome = read_sftr_tr_state_xml(tsr_path)
            .with_context(|| format!("reading companion SFTR TSR {}", tsr_path.display()))?;
        info!(records = outcome.records.len(), "loaded companion SFTR TSR");
        Some(outcome.records)
    } else {
        None
    };

    let now = Utc::now();
    let ctx = CheckContext {
        thresholds: Thresholds::default(),
        today: now.date_naive(),
        now,
    };

    let mut action_distribution: BTreeMap<String, u32> = BTreeMap::new();
    let mut event_distribution: BTreeMap<String, u32> = BTreeMap::new();
    for r in &records {
        let action = r.action_type.as_deref().unwrap_or("(missing)").to_owned();
        *action_distribution.entry(action).or_insert(0) += 1;
        let event = r.event_type.as_deref().unwrap_or("(missing)").to_owned();
        *event_distribution.entry(event).or_insert(0) += 1;
    }
    let activity_summary = TrActivitySummary {
        total_records: records.len() as u32,
        action_distribution,
        event_distribution,
    };

    // Streaming pipeline (Milestone 0.26): format + SFTR TAR issues
    // fold into one spill-capable sink; `records_processed` =
    // records.len() (Pattern A, exactly as the former `build_summary`).
    // The activity distribution sibling JSON is record-derived and
    // unchanged. Byte-identical for non-spilling inputs.
    let sink = std::sync::Mutex::new(SortedIssueSink::new(
        &ctx.thresholds,
        STREAM_SPILL_MAX_ISSUES,
    ));
    sink.lock()
        .expect("issue sink mutex")
        .push_batch(format_issues);
    stream_checks_into(&default_sftr_tr_activity_checks(), &sink, |c| {
        c.run(&records, &prior, tsr_records.as_deref(), &ctx)
    });
    let (summary, sorted) = sink.into_inner().expect("issue sink mutex").finish(
        Regime::Sftr,
        inputs.len() as u32,
        records.len() as u32,
        started_at,
        Utc::now(),
    );

    std::fs::create_dir_all(out)
        .with_context(|| format!("creating output directory {}", out.display()))?;
    write_summary_json(&out.join("summary.json"), &summary)?;
    let activity_json =
        serde_json::to_string_pretty(&activity_summary).context("serialising TrActivitySummary")?;
    std::fs::write(out.join("tr_activity_summary.json"), activity_json)
        .context("writing tr_activity_summary.json")?;
    let mut top = TopIssues::with_capacity(20);
    write_issues_csv_from_iter(
        &out.join("tr_activity_issues.csv"),
        sorted.inspect(|issue| top.offer(issue)),
    )?;
    let top = top.into_sorted();
    write_report_html(
        &out.join("tr_activity_report.html"),
        &summary,
        &top,
        &sources,
    )?;

    if let Some(path) = email_config_path {
        let cfg = opendqi_report::SmtpConfig::from_yaml_file(path)?;
        let sent = opendqi_report::send_report_email(
            &cfg,
            &summary,
            &out.join("tr_activity_report.html"),
            &out.join("summary.json"),
            &out.join("tr_activity_issues.csv"),
        )?;
        if sent {
            info!(to = ?cfg.to, "SFTR TAR report emailed");
        } else {
            info!("email config is disabled — skipped send");
        }
    }

    let critical = summary
        .issues_by_severity
        .get(&Severity::Critical)
        .copied()
        .unwrap_or(0);
    let high = summary
        .issues_by_severity
        .get(&Severity::High)
        .copied()
        .unwrap_or(0);
    println!(
        "Scanned {} SFTR TAR record(s). {} issues ({} critical, {} high). Score: {:.1}/100.",
        summary.records_processed, summary.issues_total, critical, high, summary.quality_score
    );
    println!("Report: {}", out.join("tr_activity_report.html").display());
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_tr_audit(
    tar_path: &Path,
    tsr_path: &Path,
    store_path: Option<&Path>,
    out: &Path,
    email_config_path: Option<&Path>,
) -> Result<()> {
    let started_at = Utc::now();

    // 1. Load the TAR + TSR layers (SFTR has no feedback message).
    let tar_inputs = discover_emir_inputs(tar_path)?;
    let mut tar_records: Vec<SftrRecord> = Vec::new();
    let mut tar_issues: Vec<DqIssue> = Vec::new();
    for p in &tar_inputs {
        if !has_extension(p, "xml") {
            warn!(path = %p.display(), "skipping non-XML file in SFTR TAR input");
            continue;
        }
        let mut outcome = read_sftr_xml(p)?;
        tar_records.append(&mut outcome.records);
        tar_issues.append(&mut outcome.issues);
    }
    info!(records = tar_records.len(), "loaded SFTR TAR");

    let tsr_outcome = read_sftr_tr_state_xml(tsr_path)
        .with_context(|| format!("reading SFTR TSR {}", tsr_path.display()))?;
    info!(records = tsr_outcome.records.len(), "loaded SFTR TSR");

    let prior: Vec<SftrRecord> = if let Some(sp) = store_path {
        let store = opendqi_store::open_store(sp)
            .with_context(|| format!("opening history store at {}", sp.display()))?;
        let mut utis: Vec<&str> = tar_records
            .iter()
            .filter_map(|r| r.uti.as_deref())
            .map(str::trim)
            .filter(|u| !u.is_empty())
            .collect();
        utis.extend(
            tsr_outcome
                .records
                .iter()
                .filter_map(|r| r.uti.as_deref())
                .map(str::trim)
                .filter(|u| !u.is_empty()),
        );
        let prior = store
            .load_prior_sftr(&utis, i64::MAX)
            .context("loading prior SFTR records from history store")?;
        info!(prior_records = prior.len(), "loaded prior records");
        prior
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let ctx = CheckContext {
        thresholds: Thresholds::default(),
        today: now.date_naive(),
        now,
    };

    // Streaming pipeline (Milestone 0.26): every layer's issues fold
    // into one spill-capable sink (Pattern A: `records_processed` =
    // tar_records.len(), exactly as the former `build_summary`).
    // Byte-identical for non-spilling inputs.
    let all_inputs = [tar_path.to_path_buf(), tsr_path.to_path_buf()];
    let sources: Vec<String> = all_inputs
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    let sink = std::sync::Mutex::new(SortedIssueSink::new(
        &ctx.thresholds,
        STREAM_SPILL_MAX_ISSUES,
    ));
    {
        let mut s = sink.lock().expect("issue sink mutex");
        s.push_batch(tar_issues);
        s.push_batch(tsr_outcome.issues.clone());
    }
    stream_checks_into(&default_sftr_checks(), &sink, |c| c.run(&tar_records, &ctx));
    stream_checks_into(&default_sftr_lifecycle_checks(), &sink, |c| {
        c.run(&tar_records, &prior, &ctx)
    });
    stream_checks_into(&default_sftr_tr_state_checks(), &sink, |c| {
        c.run(&tsr_outcome.records, &prior, &ctx)
    });
    stream_checks_into(&default_sftr_tr_activity_checks(), &sink, |c| {
        c.run(&tar_records, &prior, Some(&tsr_outcome.records), &ctx)
    });
    // Cross-layer coherence (SFTR.AUD.*, TAR↔TSR only).
    sink.lock().expect("issue sink mutex").push_batch(
        opendqi_core::dq::compute_tr_audit_sftr_issues(
            &tar_records,
            &tsr_outcome.records,
            &tsr_path.to_string_lossy(),
        ),
    );

    let (summary, sorted) = sink.into_inner().expect("issue sink mutex").finish(
        Regime::Sftr,
        all_inputs.len() as u32,
        tar_records.len() as u32,
        started_at,
        Utc::now(),
    );

    std::fs::create_dir_all(out)
        .with_context(|| format!("creating output directory {}", out.display()))?;
    write_summary_json(&out.join("summary.json"), &summary)?;
    let mut top = TopIssues::with_capacity(20);
    write_issues_csv_from_iter(
        &out.join("tr_audit_issues.csv"),
        sorted.inspect(|issue| top.offer(issue)),
    )?;
    let top = top.into_sorted();
    write_report_html(&out.join("tr_audit_report.html"), &summary, &top, &sources)?;

    if let Some(path) = email_config_path {
        let cfg = opendqi_report::SmtpConfig::from_yaml_file(path)?;
        let sent = opendqi_report::send_report_email(
            &cfg,
            &summary,
            &out.join("tr_audit_report.html"),
            &out.join("summary.json"),
            &out.join("tr_audit_issues.csv"),
        )?;
        if sent {
            info!(to = ?cfg.to, "SFTR TR audit report emailed");
        } else {
            info!("email config is disabled — skipped send");
        }
    }

    let critical = summary
        .issues_by_severity
        .get(&Severity::Critical)
        .copied()
        .unwrap_or(0);
    let high = summary
        .issues_by_severity
        .get(&Severity::High)
        .copied()
        .unwrap_or(0);
    println!(
        "SFTR TR audit: TAR={} TSR={}. {} issues ({} critical, {} high). Score: {:.1}/100.",
        tar_records.len(),
        tsr_outcome.records.len(),
        summary.issues_total,
        critical,
        high,
        summary.quality_score
    );
    println!("Report: {}", out.join("tr_audit_report.html").display());
    Ok(())
}

fn run_book_reconcile(
    book_path: &Path,
    tsr_path: &Path,
    mapping_path: &Path,
    out: &Path,
    email_config_path: Option<&Path>,
) -> Result<()> {
    let started_at = Utc::now();

    let mapping = CsvMapping::from_path(mapping_path)
        .with_context(|| format!("loading mapping {}", mapping_path.display()))?;
    let book_records = read_sftr_csv(book_path, &mapping)
        .with_context(|| format!("reading book CSV {}", book_path.display()))?;
    info!(records = book_records.len(), "loaded SFTR book");

    let tsr_outcome = read_sftr_tr_state_xml(tsr_path)
        .with_context(|| format!("reading SFTR TSR {}", tsr_path.display()))?;
    info!(records = tsr_outcome.records.len(), "loaded SFTR TSR");

    // Streaming pipeline (Milestone 0.26): book-vs-TSR issues fold
    // into one spill-capable sink. No `--config` ⇒ default (empty)
    // severity overrides, so the sink's override+`issue_cmp` finish
    // == the former `sort_issues` (no overrides). Pattern A:
    // `records_processed` = book_records.len(), as the former
    // `build_summary`. Byte-identical for non-spilling inputs.
    let inputs = [book_path.to_path_buf(), tsr_path.to_path_buf()];
    let sources: Vec<String> = inputs
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    let thresholds = Thresholds::default();
    let sink = std::sync::Mutex::new(SortedIssueSink::new(&thresholds, STREAM_SPILL_MAX_ISSUES));
    {
        let mut s = sink.lock().expect("issue sink mutex");
        s.push_batch(tsr_outcome.issues.clone());
        s.push_batch(compute_sftr_book_reconcile_issues(
            &book_records,
            &tsr_outcome.records,
        ));
    }
    let (summary, sorted) = sink.into_inner().expect("issue sink mutex").finish(
        Regime::Sftr,
        inputs.len() as u32,
        book_records.len() as u32,
        started_at,
        Utc::now(),
    );

    std::fs::create_dir_all(out)
        .with_context(|| format!("creating output directory {}", out.display()))?;
    write_summary_json(&out.join("summary.json"), &summary)?;
    let mut top = TopIssues::with_capacity(20);
    write_issues_csv_from_iter(
        &out.join("book_vs_tsr_issues.csv"),
        sorted.inspect(|issue| top.offer(issue)),
    )?;
    let top = top.into_sorted();
    write_report_html(
        &out.join("book_vs_tsr_report.html"),
        &summary,
        &top,
        &sources,
    )?;

    if let Some(path) = email_config_path {
        let cfg = opendqi_report::SmtpConfig::from_yaml_file(path)?;
        let sent = opendqi_report::send_report_email(
            &cfg,
            &summary,
            &out.join("book_vs_tsr_report.html"),
            &out.join("summary.json"),
            &out.join("book_vs_tsr_issues.csv"),
        )?;
        if sent {
            info!(to = ?cfg.to, "SFTR book-vs-TSR report emailed");
        } else {
            info!("email config is disabled — skipped send");
        }
    }

    let critical = summary
        .issues_by_severity
        .get(&Severity::Critical)
        .copied()
        .unwrap_or(0);
    let high = summary
        .issues_by_severity
        .get(&Severity::High)
        .copied()
        .unwrap_or(0);
    println!(
        "SFTR book vs TSR: book={} tsr={}. {} issues ({} critical, {} high). Score: {:.1}/100.",
        book_records.len(),
        tsr_outcome.records.len(),
        summary.issues_total,
        critical,
        high,
        summary.quality_score
    );
    println!("Report: {}", out.join("book_vs_tsr_report.html").display());
    Ok(())
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod sftr_book_reconcile_tests {
    use super::*;
    use chrono::NaiveDate;
    use rust_decimal::Decimal;

    fn book_baseline(uti: &str) -> SftrRecord {
        SftrRecord {
            uti: Some(uti.into()),
            loan_value: Some(Decimal::from(1_000_000)),
            loan_currency: Some("EUR".into()),
            collateral_value: Some(Decimal::new(1_100_000, 2)),
            collateral_currency: Some("EUR".into()),
            maturity_date: NaiveDate::from_ymd_opt(2027, 6, 30),
            ..Default::default()
        }
    }
    fn tsr_baseline(uti: &str) -> SftrTrStateRecord {
        SftrTrStateRecord {
            uti: Some(uti.into()),
            loan_value: Some(Decimal::from(1_000_000)),
            loan_currency: Some("EUR".into()),
            collateral_value: Some(Decimal::new(1_100_000, 2)),
            collateral_currency: Some("EUR".into()),
            maturity_date: NaiveDate::from_ymd_opt(2027, 6, 30),
            status: Some("OUTSTANDING".into()),
            ..Default::default()
        }
    }

    #[test]
    fn baseline_match_emits_nothing() {
        let issues =
            compute_sftr_book_reconcile_issues(&[book_baseline("U1")], &[tsr_baseline("U1")]);
        assert!(issues.is_empty(), "expected no issues, got {issues:?}");
    }

    #[test]
    fn in_book_not_in_tsr() {
        let issues = compute_sftr_book_reconcile_issues(&[book_baseline("UB")], &[]);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].check_id, "SFTR.BREC.IN_BOOK_NOT_IN_TSR");
    }

    #[test]
    fn in_tsr_not_in_book_outstanding_only() {
        let mut t = tsr_baseline("UT");
        let issues = compute_sftr_book_reconcile_issues(&[], &[t.clone()]);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].check_id, "SFTR.BREC.IN_TSR_NOT_IN_BOOK");
        t.status = Some("TERMINATED".into());
        let issues = compute_sftr_book_reconcile_issues(&[], &[t]);
        assert!(issues.is_empty());
    }

    #[test]
    fn loan_mismatch() {
        let mut b = book_baseline("U1");
        b.loan_value = Some(Decimal::from(9_999_999));
        let issues = compute_sftr_book_reconcile_issues(&[b], &[tsr_baseline("U1")]);
        assert!(issues
            .iter()
            .any(|i| i.check_id == "SFTR.BREC.LOAN_MISMATCH"));
    }

    #[test]
    fn loan_currency_mismatch() {
        let mut b = book_baseline("U1");
        b.loan_currency = Some("USD".into());
        let issues = compute_sftr_book_reconcile_issues(&[b], &[tsr_baseline("U1")]);
        assert!(issues
            .iter()
            .any(|i| i.check_id == "SFTR.BREC.LOAN_CURRENCY_MISMATCH"));
    }

    #[test]
    fn collateral_mismatch_outside_tolerance() {
        let mut b = book_baseline("U1");
        // 50% off — beyond 1% tolerance.
        b.collateral_value = Some(Decimal::new(550_000, 2));
        let issues = compute_sftr_book_reconcile_issues(&[b], &[tsr_baseline("U1")]);
        assert!(issues
            .iter()
            .any(|i| i.check_id == "SFTR.BREC.COLLATERAL_MISMATCH"));
    }

    #[test]
    fn maturity_mismatch() {
        let mut b = book_baseline("U1");
        b.maturity_date = NaiveDate::from_ymd_opt(2099, 12, 31);
        let issues = compute_sftr_book_reconcile_issues(&[b], &[tsr_baseline("U1")]);
        assert!(issues
            .iter()
            .any(|i| i.check_id == "SFTR.BREC.MATURITY_MISMATCH"));
    }

    #[test]
    fn status_mismatch_book_active_tsr_terminated() {
        let mut t = tsr_baseline("U1");
        t.status = Some("TERMINATED".into());
        let issues = compute_sftr_book_reconcile_issues(&[book_baseline("U1")], &[t]);
        assert!(issues
            .iter()
            .any(|i| i.check_id == "SFTR.BREC.STATUS_MISMATCH"));
    }
}

fn xsd_violation_issue(path: &Path, violation: &XsdViolation) -> DqIssue {
    let line_label = violation
        .line
        .map(|n| format!("line={n}"))
        .unwrap_or_default();
    DqIssue {
        check_id: CHECK_XSD_VIOLATION.into(),
        regime: Regime::Sftr,
        severity: Severity::High,
        dimension: DqDimension::Validity,
        record_id: None,
        uti: None,
        field: None,
        value: if line_label.is_empty() {
            None
        } else {
            Some(line_label)
        },
        message: violation.message.clone(),
        source_file: Some(path.to_string_lossy().into_owned()),
        evidence: Vec::new(),
    }
}

fn xsd_tool_error_issue(path: &Path, message: &str) -> DqIssue {
    DqIssue {
        check_id: CHECK_XSD_TOOL_ERROR.into(),
        regime: Regime::Sftr,
        severity: Severity::Warning,
        dimension: DqDimension::Validity,
        record_id: None,
        uti: None,
        field: None,
        value: None,
        message: format!("XSD validator could not run: {message}"),
        source_file: Some(path.to_string_lossy().into_owned()),
        evidence: Vec::new(),
    }
}

struct XsdReportRow {
    source_file: String,
    line: String,
    column: String,
    message: String,
}

impl XsdReportRow {
    fn from_violation(path: &Path, v: &XsdViolation) -> Self {
        Self {
            source_file: path.to_string_lossy().into_owned(),
            line: v.line.map(|n| n.to_string()).unwrap_or_default(),
            column: v.column.map(|n| n.to_string()).unwrap_or_default(),
            message: v.message.clone(),
        }
    }
}

fn write_xsd_errors_csv(path: &Path, rows: &[XsdReportRow]) -> Result<()> {
    let mut writer = csv::WriterBuilder::new()
        .has_headers(true)
        .from_path(path)
        .with_context(|| format!("creating {}", path.display()))?;
    writer.write_record(["source_file", "line", "column", "message"])?;
    let mut sorted: Vec<&XsdReportRow> = rows.iter().collect();
    sorted.sort_by(|a, b| {
        a.source_file
            .cmp(&b.source_file)
            .then_with(|| a.line.cmp(&b.line))
    });
    for row in sorted {
        writer.write_record([
            row.source_file.as_str(),
            row.line.as_str(),
            row.column.as_str(),
            row.message.as_str(),
        ])?;
    }
    writer.flush()?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_data_quality_pack(
    tsr_path: Option<&Path>,
    tar_path: Option<&Path>,
    reconciliation_path: Option<&Path>,
    missing_collateral_path: Option<&Path>,
    msr_path: Option<&Path>,
    config_path: Option<&Path>,
    as_of_str: Option<&str>,
    out: &Path,
    email_config_path: Option<&Path>,
) -> Result<()> {
    if tsr_path.is_none()
        && tar_path.is_none()
        && reconciliation_path.is_none()
        && missing_collateral_path.is_none()
        && msr_path.is_none()
    {
        return Err(anyhow!(
            "sftr data-quality-pack: at least one of --tsr / --tar / \
             --reconciliation / --missing-collateral / --msr is required"
        ));
    }

    // Resolve thresholds.
    let thresholds = match config_path {
        Some(p) => {
            let text = std::fs::read_to_string(p)
                .with_context(|| format!("reading thresholds config {}", p.display()))?;
            serde_yaml::from_str::<Thresholds>(&text)
                .with_context(|| format!("parsing thresholds config {}", p.display()))?
        }
        None => Thresholds::default(),
    };

    // Resolve as_of (default = today UTC).
    let as_of = match as_of_str {
        Some(s) => chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .with_context(|| format!("parsing --as-of {s:?} (expected YYYY-MM-DD)"))?,
        None => Utc::now().date_naive(),
    };

    // Load each provided layer.
    let tsr_records = match tsr_path {
        Some(p) => {
            let outcome = read_sftr_tr_state_xml(p)
                .with_context(|| format!("reading SFTR TSR {}", p.display()))?;
            info!(records = outcome.records.len(), "loaded SFTR TSR");
            outcome.records
        }
        None => Vec::new(),
    };
    let mut tar_records: Vec<SftrRecord> = Vec::new();
    if let Some(p) = tar_path {
        let inputs = discover_emir_inputs(p)?;
        for q in &inputs {
            if !has_extension(q, "xml") {
                warn!(path = %q.display(), "skipping non-XML file in SFTR TAR input");
                continue;
            }
            let mut outcome = read_sftr_xml(q)?;
            tar_records.append(&mut outcome.records);
        }
        info!(records = tar_records.len(), "loaded SFTR TAR");
    }
    // v0.17 C1+D1: reconciliation + missing_collateral are now
    // consumed by the 4 reconciliation DQIs and the MCR rollup
    // DQI respectively.
    let reconciliation_records = match reconciliation_path {
        Some(p) => {
            let outcome = read_sftr_reconciliation_xml(p)
                .with_context(|| format!("reading SFTR reconciliation {}", p.display()))?;
            info!(
                records = outcome.records.len(),
                "loaded SFTR reconciliation"
            );
            outcome.records
        }
        None => Vec::new(),
    };
    let missing_collateral_records = match missing_collateral_path {
        Some(p) => {
            let outcome = read_sftr_missing_collateral_xml(p)
                .with_context(|| format!("reading SFTR missing-collateral {}", p.display()))?;
            info!(
                records = outcome.records.len(),
                "loaded SFTR missing-collateral"
            );
            outcome.records
        }
        None => Vec::new(),
    };
    // v0.17 A4 + B1' + F1': SFTR MSR (auth.085) parsed, then
    // consumed by the 4 T3 margin DQIs (B1') and 6 SFTR.T3.*
    // granular checks (F1').
    let msr_records = match msr_path {
        Some(p) => {
            let outcome = read_sftr_margin_state_xml(p)
                .with_context(|| format!("reading SFTR MSR {}", p.display()))?;
            info!(
                records = outcome.records.len(),
                "loaded SFTR MSR (auth.085)"
            );
            outcome.records
        }
        None => Vec::new(),
    };

    let inputs = SftrDqiInputs {
        msr: if msr_path.is_some() {
            Some(&msr_records)
        } else {
            None
        },
        tsr: if tsr_path.is_some() {
            Some(&tsr_records)
        } else {
            None
        },
        tar: if tar_path.is_some() {
            Some(&tar_records)
        } else {
            None
        },
        reconciliation: if reconciliation_path.is_some() {
            Some(&reconciliation_records)
        } else {
            None
        },
        missing_collateral: if missing_collateral_path.is_some() {
            Some(&missing_collateral_records)
        } else {
            None
        },
        // v0.18 A3: mar slot is plumbed but no CLI flag yet
        // (added in A6). Forced to None at this surface for now.
        mar: None,
    };

    let pack = compute_sftr_dqi_pack(inputs, MappingPresence::default(), &thresholds, as_of);

    // Source labels.
    let mut sources: Vec<String> = Vec::new();
    for (lbl, p) in [
        ("SFTR-TSR", tsr_path),
        ("SFTR-TAR", tar_path),
        ("SFTR-Recon", reconciliation_path),
        ("SFTR-MCR", missing_collateral_path),
        ("SFTR-MSR", msr_path),
    ] {
        if let Some(path) = p {
            sources.push(format!("{lbl}: {}", path.display()));
        }
    }

    // Write outputs.
    std::fs::create_dir_all(out)
        .with_context(|| format!("creating output directory {}", out.display()))?;
    write_summary_json(&out.join("summary.json"), &pack.issues_summary)?;
    write_issues_csv(&out.join("issues.csv"), &pack.issues)?;
    write_indicators_csv(&out.join("indicators.csv"), &pack.indicators)?;
    write_evidence_csv(&out.join("evidence.csv"), &pack.evidence)?;
    write_report_html_with_dqi(
        &out.join("report.html"),
        &pack.issues_summary,
        &pack.issues,
        &sources,
        Some(&pack.indicators),
    )?;

    if let Some(path) = email_config_path {
        let cfg = opendqi_report::SmtpConfig::from_yaml_file(path)?;
        let sent = opendqi_report::send_report_email(
            &cfg,
            &pack.issues_summary,
            &out.join("report.html"),
            &out.join("summary.json"),
            &out.join("issues.csv"),
        )?;
        if sent {
            info!(to = ?cfg.to, "SFTR data-quality-pack report emailed");
        } else {
            info!("email config disabled — skipped send");
        }
    }

    let computed = pack
        .indicators
        .iter()
        .filter(|i| i.status != opendqi_core::DqiStatus::NotApplicable)
        .count();
    let red = pack
        .indicators
        .iter()
        .filter(|i| i.status == opendqi_core::DqiStatus::Red)
        .count();
    let amber = pack
        .indicators
        .iter()
        .filter(|i| i.status == opendqi_core::DqiStatus::Amber)
        .count();
    println!(
        "SFTR Data Quality Pack: {}/{} indicators computed ({} red, {} amber). Granular: {} issues, score {:.1}/100.",
        computed,
        pack.indicators.len(),
        red,
        amber,
        pack.issues_summary.issues_total,
        pack.issues_summary.quality_score,
    );
    println!("Report: {}", out.join("report.html").display());
    Ok(())
}
