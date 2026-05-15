//! `opendqi sftr ...` subcommands.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use clap::Subcommand;
use opendqi_core::dq::compute_sftr_book_reconcile_issues;
use opendqi_core::dq::{
    default_sftr_checks, default_sftr_feedback_checks, default_sftr_lifecycle_checks,
    default_sftr_pre_submission_checks, default_sftr_reconciliation_checks,
    default_sftr_tr_activity_checks, default_sftr_tr_state_checks,
    default_sftr_tr_state_lifecycle_checks, finalize_issues, run_all_sftr, run_all_sftr_feedback,
    run_all_sftr_lifecycle, run_all_sftr_pre_submission, run_all_sftr_reconciliation,
    run_all_sftr_tr_activity, run_all_sftr_tr_state, run_all_sftr_tr_state_lifecycle, sort_issues,
    CheckContext,
};
use opendqi_core::{
    DqDimension, DqIssue, Regime, ScanSummary, Severity, SftrRecord, SftrTrStateRecord, Thresholds,
    TrActivitySummary,
};
use opendqi_io::{
    discover_emir_inputs, has_extension, read_sftr_csv, read_sftr_parquet, write_sftr_parquet,
    CsvMapping,
};
use opendqi_report::{write_issues_csv, write_report_html, write_summary_json};
use opendqi_xml::{
    check_wellformedness, read_sftr_feedback_xml, read_sftr_reconciliation_xml,
    read_sftr_tr_state_xml, read_sftr_xml, ExternalXmllintValidator, XsdValidator, XsdViolation,
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
    /// Ingest an SFTR Trade Repository feedback file (`auth.080`) and
    /// cross-reference each UTI against the local SQLite history
    /// store. Produces `SFTR.FBK.*` issues.
    Feedback {
        /// Path to the `auth.080` XML file received from the TR.
        input: PathBuf,
        /// Required path to the SQLite history store containing prior
        /// SFTR scans.
        #[arg(long)]
        store: PathBuf,
        /// Directory where reports are written.
        #[arg(long)]
        out: PathBuf,
        /// Optional SMTP configuration YAML — emails the SFTR
        /// feedback report. See `docs/email-notifications.md`.
        #[arg(long, value_name = "PATH")]
        email_config: Option<PathBuf>,
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
    /// Consolidated SFTR TR audit. Ingests a TAR (`auth.052`), a TSR
    /// (`auth.079`), and a feedback file (`auth.080`) together,
    /// runs every layer's checks, plus 3 cross-layer coherence
    /// checks (`SFTR.AUD.*`), and writes a single
    /// `tr_audit_report.html` consolidating all three layers.
    TrAudit {
        /// Path to the TAR `auth.052` XML file (or directory).
        #[arg(long)]
        tar: PathBuf,
        /// Path to the TSR `auth.079` XML file.
        #[arg(long)]
        tsr: PathBuf,
        /// Path to the feedback `auth.080` XML file.
        #[arg(long)]
        feedback: PathBuf,
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
        SftrAction::Feedback {
            input,
            store,
            out,
            email_config,
        } => {
            run_feedback(&input, &store, &out, email_config.as_deref())?;
            Ok(ExitCode::SUCCESS)
        }
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
            feedback,
            store,
            out,
            email_config,
        } => {
            run_tr_audit(
                &tar,
                &tsr,
                &feedback,
                store.as_deref(),
                &out,
                email_config.as_deref(),
            )?;
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

    let now = Utc::now();
    let ctx = CheckContext {
        thresholds,
        today: now.date_naive(),
        now,
    };

    let checks = default_sftr_checks();
    let mut issues = run_all_sftr(&checks, &records, &ctx);
    issues.extend(format_issues);

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
        issues.extend(lifecycle_issues);
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
        issues.extend(psc_issues);
    }

    finalize_issues(&mut issues, &ctx);

    let summary = build_summary(&records, &issues, &inputs, started_at, Utc::now());

    std::fs::create_dir_all(out)
        .with_context(|| format!("creating output directory {}", out.display()))?;
    write_summary_json(&out.join("summary.json"), &summary)?;
    write_issues_csv(&out.join("issues.csv"), &issues)?;
    write_report_html(&out.join("report.html"), &summary, &issues, &sources)?;
    if validator.is_some() {
        write_xsd_errors_csv(&out.join("xsd_errors.csv"), &xsd_rows)?;
    }

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

fn run_feedback(
    input: &Path,
    store_path: &Path,
    out: &Path,
    email_config_path: Option<&Path>,
) -> Result<()> {
    let started_at = Utc::now();
    let outcome = read_sftr_feedback_xml(input)
        .with_context(|| format!("reading feedback file {}", input.display()))?;
    info!(
        file = %input.display(),
        records = outcome.records.len(),
        format_issues = outcome.issues.len(),
        "loaded SFTR feedback XML",
    );

    let mut issues: Vec<DqIssue> = outcome.issues;

    let mut store = opendqi_store::open_store(store_path)
        .with_context(|| format!("opening history store at {}", store_path.display()))?;

    // Persist the feedback batch into the `feedbacks` table so the
    // `opendqi feedback list/resolve/stale` workflow can pick it up.
    let persisted = store
        .persist_feedback_batch(&outcome.records)
        .context("persisting SFTR feedback batch to history store")?;
    info!(persisted, "feedback rows persisted to store");

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
    let fbk_issues = run_all_sftr_feedback(
        &default_sftr_feedback_checks(),
        &outcome.records,
        &prior,
        &ctx,
    );
    info!(feedback_issues = fbk_issues.len(), "feedback checks run");
    issues.extend(fbk_issues);
    finalize_issues(&mut issues, &ctx);

    let inputs = vec![input.to_path_buf()];
    let sources = vec![input.to_string_lossy().into_owned()];
    let summary = build_feedback_summary(
        outcome.records.len(),
        &issues,
        &inputs,
        started_at,
        Utc::now(),
    );

    std::fs::create_dir_all(out)
        .with_context(|| format!("creating output directory {}", out.display()))?;
    write_summary_json(&out.join("summary.json"), &summary)?;
    write_issues_csv(&out.join("issues.csv"), &issues)?;
    write_report_html(&out.join("report.html"), &summary, &issues, &sources)?;

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
            info!(to = ?cfg.to, "SFTR feedback report emailed");
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
        "Ingested {} SFTR feedback record(s). {} issues ({} critical, {} high). Score: {:.1}/100.",
        summary.records_processed, summary.issues_total, critical, high, summary.quality_score
    );
    println!("Report: {}", out.join("report.html").display());
    Ok(())
}

fn build_feedback_summary(
    feedback_count: usize,
    issues: &[DqIssue],
    inputs: &[PathBuf],
    started_at: chrono::DateTime<Utc>,
    finished_at: chrono::DateTime<Utc>,
) -> ScanSummary {
    let mut by_sev: BTreeMap<Severity, u32> = BTreeMap::new();
    let mut by_dim: BTreeMap<DqDimension, u32> = BTreeMap::new();
    for i in issues {
        *by_sev.entry(i.severity).or_insert(0) += 1;
        *by_dim.entry(i.dimension).or_insert(0) += 1;
    }
    ScanSummary {
        regime: Regime::Sftr,
        files_processed: inputs.len() as u32,
        records_processed: feedback_count as u32,
        issues_total: issues.len() as u32,
        issues_by_severity: by_sev,
        issues_by_dimension: by_dim,
        quality_score: opendqi_core::quality_score(feedback_count as u32, issues),
        started_at,
        finished_at,
    }
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

    let mut issues: Vec<DqIssue> = outcome.issues;

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
    let rec_issues = run_all_sftr_reconciliation(
        &default_sftr_reconciliation_checks(),
        &outcome.records,
        &prior,
        &ctx,
    );
    info!(
        reconciliation_issues = rec_issues.len(),
        "reconciliation checks run"
    );
    issues.extend(rec_issues);
    finalize_issues(&mut issues, &ctx);

    let inputs = vec![input.to_path_buf()];
    let sources = vec![input.to_string_lossy().into_owned()];
    let summary = build_feedback_summary(
        outcome.records.len(),
        &issues,
        &inputs,
        started_at,
        Utc::now(),
    );

    std::fs::create_dir_all(out)
        .with_context(|| format!("creating output directory {}", out.display()))?;
    write_summary_json(&out.join("summary.json"), &summary)?;
    write_issues_csv(&out.join("issues.csv"), &issues)?;
    write_report_html(&out.join("report.html"), &summary, &issues, &sources)?;

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

    let mut issues: Vec<DqIssue> = outcome.issues;

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
    let tsr_issues = run_all_sftr_tr_state(
        &default_sftr_tr_state_checks(),
        &outcome.records,
        &prior,
        &ctx,
    );
    info!(tsr_issues = tsr_issues.len(), "SFTR TSR checks run");
    issues.extend(tsr_issues);
    if !prior_tsr.is_empty() {
        let lfc_issues = run_all_sftr_tr_state_lifecycle(
            &default_sftr_tr_state_lifecycle_checks(),
            &outcome.records,
            &prior_tsr,
            &ctx,
        );
        info!(
            lfc_issues = lfc_issues.len(),
            "SFTR TSR lifecycle checks run"
        );
        issues.extend(lfc_issues);
    }
    finalize_issues(&mut issues, &ctx);

    let inputs = vec![input.to_path_buf()];
    let sources = vec![input.to_string_lossy().into_owned()];
    let summary = build_feedback_summary(
        outcome.records.len(),
        &issues,
        &inputs,
        started_at,
        Utc::now(),
    );

    std::fs::create_dir_all(out)
        .with_context(|| format!("creating output directory {}", out.display()))?;
    write_summary_json(&out.join("summary.json"), &summary)?;
    write_issues_csv(&out.join("tr_state_issues.csv"), &issues)?;
    write_report_html(
        &out.join("tr_state_report.html"),
        &summary,
        &issues,
        &sources,
    )?;

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

    let activity_issues = run_all_sftr_tr_activity(
        &default_sftr_tr_activity_checks(),
        &records,
        &prior,
        tsr_records.as_deref(),
        &ctx,
    );
    info!(
        activity_issues = activity_issues.len(),
        "SFTR TAR checks run"
    );

    let mut issues: Vec<DqIssue> = format_issues;
    issues.extend(activity_issues);
    finalize_issues(&mut issues, &ctx);

    let summary = build_summary(&records, &issues, &inputs, started_at, Utc::now());

    std::fs::create_dir_all(out)
        .with_context(|| format!("creating output directory {}", out.display()))?;
    write_summary_json(&out.join("summary.json"), &summary)?;
    let activity_json =
        serde_json::to_string_pretty(&activity_summary).context("serialising TrActivitySummary")?;
    std::fs::write(out.join("tr_activity_summary.json"), activity_json)
        .context("writing tr_activity_summary.json")?;
    write_issues_csv(&out.join("tr_activity_issues.csv"), &issues)?;
    write_report_html(
        &out.join("tr_activity_report.html"),
        &summary,
        &issues,
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
    feedback_path: &Path,
    store_path: Option<&Path>,
    out: &Path,
    email_config_path: Option<&Path>,
) -> Result<()> {
    use std::collections::HashSet;

    let started_at = Utc::now();

    // 1. Load all three SFTR layers.
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

    let feedback_outcome = read_sftr_feedback_xml(feedback_path)
        .with_context(|| format!("reading SFTR feedback {}", feedback_path.display()))?;
    info!(
        records = feedback_outcome.records.len(),
        "loaded SFTR feedback"
    );

    let mut issues: Vec<DqIssue> = tar_issues;
    issues.extend(tsr_outcome.issues.clone());
    issues.extend(feedback_outcome.issues.clone());

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
        utis.extend(
            feedback_outcome
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

    let tar_checks = run_all_sftr(&default_sftr_checks(), &tar_records, &ctx);
    issues.extend(tar_checks);
    let lifecycle =
        run_all_sftr_lifecycle(&default_sftr_lifecycle_checks(), &tar_records, &prior, &ctx);
    issues.extend(lifecycle);
    let tsr_checks = run_all_sftr_tr_state(
        &default_sftr_tr_state_checks(),
        &tsr_outcome.records,
        &prior,
        &ctx,
    );
    issues.extend(tsr_checks);
    let fbk_checks = run_all_sftr_feedback(
        &default_sftr_feedback_checks(),
        &feedback_outcome.records,
        &prior,
        &ctx,
    );
    issues.extend(fbk_checks);
    let activity_checks = run_all_sftr_tr_activity(
        &default_sftr_tr_activity_checks(),
        &tar_records,
        &prior,
        Some(&tsr_outcome.records),
        &ctx,
    );
    issues.extend(activity_checks);

    // Cross-layer coherence (SFTR.AUD.*).
    let tsr_utis: HashSet<&str> = tsr_outcome
        .records
        .iter()
        .filter_map(|r| r.uti.as_deref())
        .map(str::trim)
        .filter(|u| !u.is_empty())
        .collect();
    let tar_utis: HashSet<&str> = tar_records
        .iter()
        .filter_map(|r| r.uti.as_deref())
        .map(str::trim)
        .filter(|u| !u.is_empty())
        .collect();
    let rejected_utis: HashSet<&str> = feedback_outcome
        .records
        .iter()
        .filter(|f| matches!(f.feedback_type, opendqi_core::FeedbackType::Rejected))
        .filter_map(|f| f.uti.as_deref())
        .map(str::trim)
        .filter(|u| !u.is_empty())
        .collect();
    let tsr_outstanding_utis: HashSet<&str> = tsr_outcome
        .records
        .iter()
        .filter(|r| {
            r.status
                .as_deref()
                .map(|s| {
                    let s = s.trim();
                    s.is_empty()
                        || s.eq_ignore_ascii_case("OUTSTANDING")
                        || s.eq_ignore_ascii_case("ACTIVE")
                })
                .unwrap_or(true)
        })
        .filter_map(|r| r.uti.as_deref())
        .map(str::trim)
        .filter(|u| !u.is_empty())
        .collect();

    for r in &tar_records {
        let is_newt = r
            .action_type
            .as_deref()
            .map(|a| a.eq_ignore_ascii_case("NEWT"))
            .unwrap_or(false);
        if !is_newt {
            continue;
        }
        if let Some(uti) = r.uti.as_deref() {
            let uti = uti.trim();
            if !uti.is_empty() && !tsr_utis.contains(uti) {
                issues.push(DqIssue {
                    check_id: "SFTR.AUD.NEWT_IN_TAR_NOT_IN_TSR".into(),
                    regime: Regime::Sftr,
                    severity: Severity::High,
                    dimension: DqDimension::Consistency,
                    record_id: r.record_id.clone(),
                    uti: Some(uti.to_owned()),
                    field: Some("uti".into()),
                    value: Some(uti.to_owned()),
                    message: format!(
                        "SFT UTI {uti} was NEWT'd in the TAR but is absent from the TSR — submission may not have been accepted."
                    ),
                    source_file: r.source_file.clone(),
                    evidence: Vec::new(),
                });
            }
        }
    }
    for uti in tsr_outstanding_utis.iter() {
        if !tar_utis.contains(uti) {
            issues.push(DqIssue {
                check_id: "SFTR.AUD.OUTSTANDING_IN_TSR_NOT_IN_TAR".into(),
                regime: Regime::Sftr,
                severity: Severity::Warning,
                dimension: DqDimension::Consistency,
                record_id: None,
                uti: Some((*uti).to_owned()),
                field: Some("uti".into()),
                value: Some((*uti).to_owned()),
                message: format!(
                    "SFT UTI {uti} is outstanding in the TSR but no record appears in the TAR for this period."
                ),
                source_file: Some(tsr_path.to_string_lossy().into_owned()),
                evidence: Vec::new(),
            });
        }
    }
    for uti in rejected_utis.iter() {
        if tsr_outstanding_utis.contains(uti) {
            issues.push(DqIssue {
                check_id: "SFTR.AUD.REJECTED_BUT_OUTSTANDING_IN_TSR".into(),
                regime: Regime::Sftr,
                severity: Severity::Critical,
                dimension: DqDimension::Consistency,
                record_id: None,
                uti: Some((*uti).to_owned()),
                field: Some("uti".into()),
                value: Some((*uti).to_owned()),
                message: format!(
                    "SFT UTI {uti} is reported rejected in the feedback file yet appears outstanding in the TSR — TR-side inconsistency."
                ),
                source_file: Some(feedback_path.to_string_lossy().into_owned()),
                evidence: Vec::new(),
            });
        }
    }

    finalize_issues(&mut issues, &ctx);

    let all_inputs = vec![
        tar_path.to_path_buf(),
        tsr_path.to_path_buf(),
        feedback_path.to_path_buf(),
    ];
    let sources: Vec<String> = all_inputs
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    let summary = build_summary(&tar_records, &issues, &all_inputs, started_at, Utc::now());

    std::fs::create_dir_all(out)
        .with_context(|| format!("creating output directory {}", out.display()))?;
    write_summary_json(&out.join("summary.json"), &summary)?;
    write_issues_csv(&out.join("tr_audit_issues.csv"), &issues)?;
    write_report_html(
        &out.join("tr_audit_report.html"),
        &summary,
        &issues,
        &sources,
    )?;

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
        "SFTR TR audit: TAR={} TSR={} feedback={}. {} issues ({} critical, {} high). Score: {:.1}/100.",
        tar_records.len(),
        tsr_outcome.records.len(),
        feedback_outcome.records.len(),
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

    let mut issues: Vec<DqIssue> = tsr_outcome.issues.clone();
    issues.extend(compute_sftr_book_reconcile_issues(
        &book_records,
        &tsr_outcome.records,
    ));
    sort_issues(&mut issues);

    let inputs = vec![book_path.to_path_buf(), tsr_path.to_path_buf()];
    let sources: Vec<String> = inputs
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    let summary = build_summary(&book_records, &issues, &inputs, started_at, Utc::now());

    std::fs::create_dir_all(out)
        .with_context(|| format!("creating output directory {}", out.display()))?;
    write_summary_json(&out.join("summary.json"), &summary)?;
    write_issues_csv(&out.join("book_vs_tsr_issues.csv"), &issues)?;
    write_report_html(
        &out.join("book_vs_tsr_report.html"),
        &summary,
        &issues,
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

fn build_summary(
    records: &[SftrRecord],
    issues: &[DqIssue],
    inputs: &[PathBuf],
    started_at: chrono::DateTime<Utc>,
    finished_at: chrono::DateTime<Utc>,
) -> ScanSummary {
    let mut by_sev: BTreeMap<Severity, u32> = BTreeMap::new();
    let mut by_dim: BTreeMap<DqDimension, u32> = BTreeMap::new();
    for i in issues {
        *by_sev.entry(i.severity).or_insert(0) += 1;
        *by_dim.entry(i.dimension).or_insert(0) += 1;
    }
    ScanSummary {
        regime: Regime::Sftr,
        files_processed: inputs.len() as u32,
        records_processed: records.len() as u32,
        issues_total: issues.len() as u32,
        issues_by_severity: by_sev,
        issues_by_dimension: by_dim,
        quality_score: opendqi_core::quality_score(records.len() as u32, issues),
        started_at,
        finished_at,
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
