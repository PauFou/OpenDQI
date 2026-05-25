//! `opendqi emir ...` subcommands.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use clap::Subcommand;
use opendqi_core::dq::compute_book_reconcile_issues;
use opendqi_core::dq::{
    default_checks, default_feedback_checks, default_lifecycle_checks,
    default_margin_activity_checks, default_margin_activity_lifecycle_checks,
    default_margin_state_checks, default_margin_state_lifecycle_checks,
    default_pre_submission_checks, default_recon_stats_checks, default_reconciliation_checks,
    default_tr_activity_checks, default_tr_state_checks, default_tr_state_lifecycle_checks,
    default_warnings_checks, default_warnings_counterparty_checks,
    default_warnings_transaction_checks, run_all_lifecycle, run_all_pre_submission, CheckContext,
};
use opendqi_core::{
    compute_emir_dqi_pack, stream_checks_into, stream_emir_checks_into, DqDimension, DqIssue,
    EmirDqiInputs, EmirRecord, MappingPresence, MarginActivityRecord, MarginStateRecord, Regime,
    Severity, SortedIssueSink, Thresholds, TrActivitySummary, TrStateRecord,
    STREAM_SPILL_MAX_ISSUES,
};
use opendqi_io::{
    discover_emir_inputs, has_extension, read_emir_csv, read_emir_parquet, write_emir_parquet,
    CsvMapping,
};
use opendqi_report::{
    write_evidence_csv, write_indicators_csv, write_issues_csv, write_issues_csv_from_iter,
    write_report_html, write_report_html_with_dqi, write_summary_json, TopIssues,
};
use opendqi_xml::{
    check_wellformedness, read_emir_feedback_xml, read_emir_mar_xml, read_emir_msr_xml,
    read_emir_position_set_xml, read_emir_recon_stats_xml, read_emir_tr_state_xml,
    read_emir_warnings_xml, read_emir_xml, ExternalXmllintValidator, XsdValidator, XsdViolation,
};
use tracing::{info, warn};

const CHECK_XSD_VIOLATION: &str = "EMIR.FMT.XSD_VIOLATION";
const CHECK_XSD_TOOL_ERROR: &str = "EMIR.FMT.XSD_TOOL_ERROR";

#[derive(Subcommand)]
pub enum EmirAction {
    /// Run the full DQ scan over a CSV or XML input (file or directory).
    Scan {
        /// Path to a CSV/XML file or a directory containing such files.
        input: PathBuf,
        /// Path to the YAML mapping describing CSV columns. Required
        /// when the input set contains at least one CSV file.
        #[arg(long)]
        mapping: Option<PathBuf>,
        /// Directory where reports are written. Created if absent.
        #[arg(long)]
        out: PathBuf,
        /// Optional YAML thresholds configuration.
        #[arg(long)]
        config: Option<PathBuf>,
        /// Optional XSD schema. When set, every XML input is also
        /// validated against the schema and violations are added to
        /// the report (one `EMIR.FMT.XSD_VIOLATION` per error line).
        #[arg(long)]
        xsd: Option<PathBuf>,
        /// Optional SQLite history-store path. When set, scanned
        /// records are persisted and cross-batch lifecycle checks
        /// (MODI-without-NEWT, duplicate NEWT, valuation regression,
        /// valuation-after-termination) run against the accumulated
        /// history.
        #[arg(long, value_name = "PATH")]
        store: Option<PathBuf>,
        /// Optional path to a `rejection_profile.yml` exported by
        /// `opendqi feedback analytics`. When set, the scan runs the
        /// pre-submission `EMIR.PSC.*` family that flags records
        /// likely to be rejected by the TR based on historical
        /// patterns. See `docs/pre-submission-checks.md`.
        #[arg(long, value_name = "PATH")]
        rejection_profile: Option<PathBuf>,
        /// Optional SMTP configuration YAML. When set, the HTML
        /// report + `summary.json` + `issues.csv` are emailed to the
        /// recipients listed in the config. The SMTP password is
        /// read from the env var named in the config
        /// (`OPENDQI_SMTP_PASS` by default). See
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
    /// Ingest a Trade Repository feedback file (ISO 20022 `auth.092`,
    /// "Missing / Inaccurate Trade Reports") and cross-reference each
    /// UTI against the local SQLite history store. Produces
    /// `EMIR.FBK.*` issues — confirmed gaps, rejected submissions,
    /// inaccurate fields, TR/firm discrepancies.
    Feedback {
        /// Path to the `auth.092` XML file received from the TR.
        input: PathBuf,
        /// Required path to the SQLite history store containing
        /// prior scans.
        #[arg(long)]
        store: PathBuf,
        /// Directory where reports are written.
        #[arg(long)]
        out: PathBuf,
        /// Optional SMTP configuration YAML — emails the feedback
        /// report after writing it. See `docs/email-notifications.md`.
        #[arg(long, value_name = "PATH")]
        email_config: Option<PathBuf>,
    },
    /// Ingest an EMIR Trade Activity Report (TR replay of
    /// `auth.030`) and produce `EMIR.TRA.*` issues: distribution
    /// histograms, repeated correction detection, NEWT/MODI/TERM
    /// spikes, duplicate NEWT in batch. With `--tsr <auth.107>`
    /// also runs TAR↔TSR coherence checks. Outputs
    /// `summary.json`, `tr_activity_issues.csv`,
    /// `tr_activity_report.html`.
    TrActivityScan {
        /// Path to the `auth.030` XML file (firm-submitted or TR
        /// replay) — directory accepted.
        input: PathBuf,
        /// Optional path to the SQLite history store.
        #[arg(long)]
        store: Option<PathBuf>,
        /// Optional companion TSR (`auth.107`). When set, triggers
        /// the TAR↔TSR coherence check.
        #[arg(long)]
        tsr: Option<PathBuf>,
        /// Directory where reports are written.
        #[arg(long)]
        out: PathBuf,
        /// Optional SMTP configuration YAML — emails the TAR report
        /// after writing it. See `docs/email-notifications.md`.
        #[arg(long, value_name = "PATH")]
        email_config: Option<PathBuf>,
    },
    /// Ingest an EMIR Trade State Report (ISO 20022 `auth.107`) and
    /// produce `EMIR.TST.*` issues over the TR's snapshot:
    /// outstanding summary, stale / missing valuation, active past
    /// maturity, placeholder maturity, duplicate active UTI,
    /// valuation after termination. The state layer is reported
    /// independently from the activity layer; outputs are
    /// `summary.json`, `tr_state_issues.csv`, `tr_state_report.html`.
    TrStateScan {
        /// Path to the `auth.107` XML file received from the TR.
        input: PathBuf,
        /// Optional path to the SQLite history store (enriches the
        /// analysis with submission history when set).
        #[arg(long)]
        store: Option<PathBuf>,
        /// Directory where reports are written.
        #[arg(long)]
        out: PathBuf,
        /// Optional SMTP configuration YAML. When set, the HTML
        /// report + summary.json + issues.csv are emailed to the
        /// recipients listed in the config after the scan. See
        /// `docs/email-notifications.md`.
        #[arg(long, value_name = "PATH")]
        email_config: Option<PathBuf>,
    },
    /// Consolidated TR audit. Ingests a TAR (`auth.030`), a TSR
    /// (`auth.107`), and a feedback file (`auth.092`) together,
    /// runs every layer's checks, plus 3 cross-layer coherence
    /// checks (`EMIR.AUD.*`), and writes a single
    /// `tr_audit_report.html` consolidating all three layers.
    /// `--store` is optional and enables lifecycle checks on the
    /// TAR records.
    TrAudit {
        /// Path to the TAR `auth.030` XML file (or directory).
        #[arg(long)]
        tar: PathBuf,
        /// Path to the TSR `auth.107` XML file.
        #[arg(long)]
        tsr: PathBuf,
        /// Path to the feedback `auth.092` XML file.
        #[arg(long)]
        feedback: PathBuf,
        /// Optional path to the SQLite history store.
        #[arg(long)]
        store: Option<PathBuf>,
        /// Directory where reports are written.
        #[arg(long)]
        out: PathBuf,
        /// Optional SMTP configuration YAML. When set, the consolidated
        /// `tr_audit_report.html` is emailed to the recipients listed
        /// in the config. See `docs/email-notifications.md`.
        #[arg(long, value_name = "PATH")]
        email_config: Option<PathBuf>,
    },
    /// EMIR Article 11 collateral obligation: cross-reference the
    /// **TSR** (`auth.107`, trade state) against the **MSR**
    /// (`auth.109`, margin state). Joins on UTI and emits
    /// `EMIR.COL.MISSING` (no MSR link, or all four IM/VM
    /// posted+collected absent/zero) and `EMIR.COL.STALE` (linked
    /// non-zero MSR snapshot older than
    /// `emir_rmt.collateral_max_age_days`, default 1, vs the TSR
    /// snapshot). Outputs `summary.json`,
    /// `collateral_audit_issues.csv`, `collateral_audit_report.html`.
    CollateralAudit {
        /// Path to the TSR `auth.107` XML file.
        #[arg(long)]
        tsr: PathBuf,
        /// Path to the MSR `auth.109` XML file.
        #[arg(long)]
        msr: PathBuf,
        /// Optional path to the SQLite history store. Reserved for
        /// future cross-batch enrichments — currently unused by this
        /// command (cross-message single-snapshot only).
        #[arg(long)]
        store: Option<PathBuf>,
        /// Optional YAML config overriding `Thresholds`
        /// (notably `emir_rmt.collateral_max_age_days`).
        #[arg(long)]
        config: Option<PathBuf>,
        /// Directory where reports are written.
        #[arg(long)]
        out: PathBuf,
        /// Optional SMTP configuration YAML. When set, the
        /// `collateral_audit_report.html` is emailed.
        #[arg(long, value_name = "PATH")]
        email_config: Option<PathBuf>,
    },
    /// Reconcile a firm's internal booking system export against a
    /// TR Trade State Report (auth.107). Produces `EMIR.BREC.*`
    /// issues for missing UTIs (in either direction) and field
    /// mismatches (notional, currency, valuation, maturity, status).
    /// Outputs `summary.json`, `book_vs_tsr_issues.csv`,
    /// `book_vs_tsr_report.html`.
    BookReconcile {
        /// Path to the internal book export (CSV).
        #[arg(long)]
        book: PathBuf,
        /// Path to the TR `auth.107` Trade State Report XML.
        #[arg(long)]
        tsr: PathBuf,
        /// Path to the YAML mapping describing book CSV columns.
        #[arg(long)]
        mapping: PathBuf,
        /// Directory where reports are written.
        #[arg(long)]
        out: PathBuf,
        /// Optional SMTP configuration YAML — emails the book-vs-TSR
        /// report. See `docs/email-notifications.md`.
        #[arg(long, value_name = "PATH")]
        email_config: Option<PathBuf>,
    },
    /// Ingest an EMIR Margin Activity Report (ISO 20022 `auth.108`)
    /// and produce `EMIR.MAR.*` issues: action-type enum, negative
    /// posted/collected margin, large margin delta, missing
    /// margin_currency, missing portfolio code, reporting timeliness,
    /// duplicate margin calls. Outputs `summary.json`,
    /// `mar_issues.csv`, `mar_report.html`.
    MarScan {
        /// Path to the `auth.108` XML file.
        input: PathBuf,
        /// Optional path to the SQLite history store.
        #[arg(long)]
        store: Option<PathBuf>,
        /// Directory where reports are written.
        #[arg(long)]
        out: PathBuf,
        /// Optional SMTP configuration YAML — emails the MAR report.
        /// See `docs/email-notifications.md`.
        #[arg(long, value_name = "PATH")]
        email_config: Option<PathBuf>,
    },
    /// Ingest an EMIR Margin State Report (ISO 20022 `auth.109`)
    /// and produce `EMIR.MSR.*` issues over the TR's margin snapshot:
    /// negative IM/VM/collateral, stale snapshot, missing margin on
    /// outstanding UTI, haircut out of range, collateralisation
    /// category enum, IM posted/collected imbalance. Outputs
    /// `summary.json`, `msr_issues.csv`, `msr_report.html`.
    MsrScan {
        /// Path to the `auth.109` XML file.
        input: PathBuf,
        /// Optional path to the SQLite history store.
        #[arg(long)]
        store: Option<PathBuf>,
        /// Directory where reports are written.
        #[arg(long)]
        out: PathBuf,
        /// Optional SMTP configuration YAML — emails the MSR report.
        /// See `docs/email-notifications.md`.
        #[arg(long, value_name = "PATH")]
        email_config: Option<PathBuf>,
    },
    /// Ingest an EMIR Reconciliation Statistics report (ISO 20022
    /// `auth.091`) and produce `EMIR.RST.*` issues: low pairing /
    /// reconciliation rate, outstanding unpaired ceiling breach, and
    /// (optional) batch-over-batch pairing rate trend. Outputs
    /// `summary.json`, `recon_stats_issues.csv`,
    /// `recon_stats_report.html`. See `docs/emir-recon-stats.md`.
    ReconStats {
        /// Path to the `auth.091` XML file.
        input: PathBuf,
        /// Optional path to a previous `auth.091` XML — when set, the
        /// trend check fires when the pairing rate drops by ≥ 5pp
        /// versus the prior batch (per counterparty).
        #[arg(long)]
        prior: Option<PathBuf>,
        /// Optional config YAML overriding default thresholds.
        #[arg(long)]
        config: Option<PathBuf>,
        /// Directory where reports are written.
        #[arg(long)]
        out: PathBuf,
        /// Optional SMTP configuration YAML — emails the recon-stats
        /// report after writing it. See `docs/email-notifications.md`.
        #[arg(long, value_name = "PATH")]
        email_config: Option<PathBuf>,
    },
    /// Ingest an EMIR Data-Quality Warnings Report (ISO 20022
    /// `auth.106`) and produce `EMIR.WRN.*` issues when the
    /// missing-valuation / missing-margin-info / abnormal-values
    /// rates exceed configurable thresholds. Outputs `summary.json`,
    /// `warnings_issues.csv`, `warnings_report.html`.
    Warnings {
        /// Path to the `auth.106` XML file received from the TR.
        input: PathBuf,
        /// Optional config YAML overriding default thresholds.
        #[arg(long)]
        config: Option<PathBuf>,
        /// Directory where reports are written.
        #[arg(long)]
        out: PathBuf,
        /// Optional SMTP configuration YAML — emails the warnings
        /// report after writing it. See `docs/email-notifications.md`.
        #[arg(long, value_name = "PATH")]
        email_config: Option<PathBuf>,
    },
    /// EMIR Data Quality Pack (v0.15). Aggregates the 5 EMIR TR
    /// layers (TSR / TAR / MSR / MAR / Feedback) into 10
    /// regulator-style indicators (numerator / denominator / rate
    /// / threshold / status). Each indicator gets ≤ 20 evidence
    /// rows for drill-down. The granular 216-check stream is
    /// **co-produced** (not replaced) — `issues.csv` carries the
    /// per-row defects, `indicators.csv` carries the aggregated
    /// metrics. See `docs/data-quality-pack.md` (added in D9).
    ///
    /// All input flags are optional ; at least one is required.
    /// Indicators that lack their input layer (or whose gated
    /// field is not mapped) report `status: not_applicable` with
    /// no rate and no evidence.
    DataQualityPack {
        /// Path to the TSR `auth.107` XML file.
        #[arg(long)]
        tsr: Option<PathBuf>,
        /// Path to the TAR `auth.030` XML file (or directory).
        #[arg(long)]
        tar: Option<PathBuf>,
        /// Path to the MSR `auth.109` XML file.
        #[arg(long)]
        msr: Option<PathBuf>,
        /// Path to the MAR `auth.108` XML file.
        #[arg(long)]
        mar: Option<PathBuf>,
        /// Path to the feedback `auth.092` XML file.
        #[arg(long)]
        feedback: Option<PathBuf>,
        /// Path to the `auth.090` Derivatives Trade Position Set
        /// Report. Powers 4 EMIR Position DQIs (NOTIONAL_MISSING,
        /// MARK_TO_MARKET_MISSING, NOTIONAL_NEGATIVE,
        /// COLLATERAL_MISSING) AND 4 granular EMIR.POS.*
        /// per-record checks. TR-side aggregated exposures across
        /// 4 position-set kinds (v0.18+).
        #[arg(long)]
        positions: Option<PathBuf>,
        /// Path to the `auth.091` Reconciliation Statistics XML.
        /// **Dual-purpose**: parses both the per-counterparty
        /// `ReconStatsRecord` rows AND the per-transaction
        /// `ReconciliationRecord` detail from the same file,
        /// activating the 4 v0.16 B1 cross-CP DQIs (PAIRING_RATE,
        /// RECONCILIATION_RATE, UNPAIRED_TRADES_RATE,
        /// FIELD_MISMATCH_RATE) plus the 3 v0.16 B4 reconciliation-
        /// driven DQIs (NOTIONAL_INCONSISTENT,
        /// MARGIN_INCONSISTENT_PRE_HAIRCUT,
        /// MARGIN_INCONSISTENT_POST_HAIRCUT) — 7 DQIs that
        /// self-report `not_applicable` until this flag is set.
        #[arg(long)]
        recon_stats: Option<PathBuf>,
        /// Optional YAML thresholds configuration (overrides the
        /// shipped DQI defaults via the `dqi:` block).
        #[arg(long)]
        config: Option<PathBuf>,
        /// Reference date for age-based indicators (stale
        /// valuation, stale collateral state). Defaults to today
        /// (UTC). Format: `YYYY-MM-DD`.
        #[arg(long)]
        as_of: Option<String>,
        /// Directory where outputs are written. Created if absent.
        /// Writes `report.html`, `summary.json`, `issues.csv`,
        /// `indicators.csv`, `evidence.csv`.
        #[arg(long)]
        out: PathBuf,
        /// Optional SMTP configuration YAML. When set, the
        /// `report.html` is emailed to the recipients listed in
        /// the config. See `docs/email-notifications.md`.
        #[arg(long, value_name = "PATH")]
        email_config: Option<PathBuf>,
    },
    /// Normalize EMIR XML/CSV input into a canonical Parquet file
    /// (Snappy-compressed). Schema is stable and analytics-friendly
    /// (DuckDB / Polars / PyArrow). See `docs/parquet-normalize.md`.
    Normalize {
        /// Path to an XML/CSV file or a directory of such files.
        input: PathBuf,
        /// Path to the YAML mapping describing CSV columns. Required
        /// when the input set contains at least one CSV file.
        #[arg(long)]
        mapping: Option<PathBuf>,
        /// Output Parquet file path (single file, not a directory).
        #[arg(long)]
        out: PathBuf,
    },
}

pub fn run(action: EmirAction) -> Result<ExitCode> {
    match action {
        EmirAction::Scan {
            input,
            mapping,
            out,
            config,
            xsd,
            store,
            rejection_profile,
            email_config,
        } => {
            run_scan(
                &input,
                mapping.as_deref(),
                &out,
                config.as_deref(),
                xsd.as_deref(),
                store.as_deref(),
                rejection_profile.as_deref(),
                email_config.as_deref(),
            )?;
            Ok(ExitCode::SUCCESS)
        }
        EmirAction::Validate { input, xsd } => run_validate(&input, &xsd),
        EmirAction::Feedback {
            input,
            store,
            out,
            email_config,
        } => {
            run_feedback(&input, &store, &out, email_config.as_deref())?;
            Ok(ExitCode::SUCCESS)
        }
        EmirAction::TrStateScan {
            input,
            store,
            out,
            email_config,
        } => {
            run_tr_state_scan(&input, store.as_deref(), &out, email_config.as_deref())?;
            Ok(ExitCode::SUCCESS)
        }
        EmirAction::TrActivityScan {
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
        EmirAction::TrAudit {
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
        EmirAction::CollateralAudit {
            tsr,
            msr,
            store,
            config,
            out,
            email_config,
        } => {
            run_collateral_audit(
                &tsr,
                &msr,
                store.as_deref(),
                config.as_deref(),
                &out,
                email_config.as_deref(),
            )?;
            Ok(ExitCode::SUCCESS)
        }
        EmirAction::BookReconcile {
            book,
            tsr,
            mapping,
            out,
            email_config,
        } => {
            run_book_reconcile(&book, &tsr, &mapping, &out, email_config.as_deref())?;
            Ok(ExitCode::SUCCESS)
        }
        EmirAction::MarScan {
            input,
            store,
            out,
            email_config,
        } => {
            run_mar_scan(&input, store.as_deref(), &out, email_config.as_deref())?;
            Ok(ExitCode::SUCCESS)
        }
        EmirAction::MsrScan {
            input,
            store,
            out,
            email_config,
        } => {
            run_msr_scan(&input, store.as_deref(), &out, email_config.as_deref())?;
            Ok(ExitCode::SUCCESS)
        }
        EmirAction::ReconStats {
            input,
            prior,
            config,
            out,
            email_config,
        } => {
            run_recon_stats(
                &input,
                prior.as_deref(),
                config.as_deref(),
                &out,
                email_config.as_deref(),
            )?;
            Ok(ExitCode::SUCCESS)
        }
        EmirAction::Warnings {
            input,
            config,
            out,
            email_config,
        } => {
            run_warnings(&input, config.as_deref(), &out, email_config.as_deref())?;
            Ok(ExitCode::SUCCESS)
        }
        EmirAction::DataQualityPack {
            tsr,
            tar,
            msr,
            mar,
            feedback,
            positions,
            recon_stats,
            config,
            as_of,
            out,
            email_config,
        } => {
            run_data_quality_pack(
                tsr.as_deref(),
                tar.as_deref(),
                msr.as_deref(),
                mar.as_deref(),
                feedback.as_deref(),
                positions.as_deref(),
                recon_stats.as_deref(),
                config.as_deref(),
                as_of.as_deref(),
                &out,
                email_config.as_deref(),
            )?;
            Ok(ExitCode::SUCCESS)
        }
        EmirAction::Normalize {
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
    mapping_path: Option<&Path>,
    out: &Path,
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
        return Err(anyhow!("no CSV or XML inputs found at {}", input.display()));
    }
    info!(count = inputs.len(), "discovered inputs");

    let has_csv = inputs.iter().any(|p| has_extension(p, "csv"));
    let mapping = if has_csv {
        let path = mapping_path.ok_or_else(|| {
            anyhow!("--mapping is required because the input contains at least one CSV file")
        })?;
        Some(CsvMapping::from_path(path)?)
    } else {
        None
    };

    let validator = xsd_path.map(|p| ExternalXmllintValidator::new(p.to_path_buf()));
    crate::memtrace::mem_trace("post_discovery");
    // Sampler covers parse→report (discovery is sub-second / ~MiB; the
    // GB-scale peak we hunt is downstream). RAII: named binding so it
    // lives to fn end; Drop stops+joins+prints the true-peak line.
    let _peak_sampler = crate::memtrace::start_peak_sampler();

    let mut records: Vec<EmirRecord> = Vec::new();
    let mut format_issues: Vec<DqIssue> = Vec::new();
    let mut xsd_rows: Vec<XsdReportRow> = Vec::new();
    let mut sources: Vec<String> = Vec::with_capacity(inputs.len());

    for path in &inputs {
        sources.push(path.to_string_lossy().into_owned());
        if has_extension(path, "csv") {
            let mapping = mapping
                .as_ref()
                .expect("mapping is loaded when CSV inputs are present");
            let mut batch = read_emir_csv(path, mapping)?;
            info!(file = %path.display(), rows = batch.len(), "loaded CSV");
            records.append(&mut batch);
        } else if has_extension(path, "xml") {
            let mut outcome = read_emir_xml(path)?;
            info!(
                file = %path.display(),
                trades = outcome.records.len(),
                format_issues = outcome.issues.len(),
                "loaded XML",
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
        } else if has_extension(path, "parquet") {
            let mut pq_records = read_emir_parquet(path)?;
            info!(
                file = %path.display(),
                records = pq_records.len(),
                "loaded EMIR Parquet",
            );
            records.append(&mut pq_records);
        } else {
            warn!(path = %path.display(), "unsupported file extension; skipping");
        }
    }

    crate::memtrace::mem_trace("post_parse");

    let now = Utc::now();
    let ctx = CheckContext {
        thresholds,
        today: now.date_naive(),
        now,
    };

    let checks = default_checks();
    // Streaming pipeline (Milestone 0.23): every issue source is
    // pushed into one spill-capable sink instead of materialising a
    // union `Vec<DqIssue>`. The sink owns the online aggregator
    // (summary) and produces issues in exact `issue_cmp` order at
    // `finish`. Small inputs never spill ⇒ output byte-identical.
    let sink = std::sync::Mutex::new(SortedIssueSink::new(
        &ctx.thresholds,
        STREAM_SPILL_MAX_ISSUES,
    ));
    stream_emir_checks_into(&checks, &records, &ctx, &sink);
    sink.lock()
        .expect("issue sink mutex")
        .push_batch(format_issues);
    crate::memtrace::mem_trace("post_checks");

    if let Some(store_path) = store_path {
        let mut store = opendqi_store::open_store(store_path)
            .with_context(|| format!("opening history store at {}", store_path.display()))?;
        let scan_id = store
            .persist_emir_batch(inputs.len(), &records)
            .context("persisting EMIR batch to history store")?;
        let utis: Vec<&str> = records
            .iter()
            .filter_map(|r| r.uti.as_deref())
            .map(str::trim)
            .filter(|u| !u.is_empty())
            .collect();
        let prior = store
            .load_prior_emir(&utis, scan_id)
            .context("loading prior EMIR records from history store")?;
        info!(prior_records = prior.len(), "loaded prior records");
        let lifecycle_issues =
            run_all_lifecycle(&default_lifecycle_checks(), &records, &prior, &ctx);
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
        let psc_issues =
            run_all_pre_submission(&default_pre_submission_checks(), &records, &profile, &ctx);
        info!(psc_issues = psc_issues.len(), "pre-submission checks run");
        sink.lock()
            .expect("issue sink mutex")
            .push_batch(psc_issues);
    }

    crate::memtrace::mem_trace("post_lifecycle_presub");

    let (summary, sorted) = sink.into_inner().expect("issue sink mutex").finish(
        Regime::Emir,
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
    // Single streamed pass: write each row in `issue_cmp` order while
    // a bounded selector picks the report's top-20 (no resident Vec).
    let mut top = TopIssues::with_capacity(20);
    write_issues_csv_from_iter(
        &out.join("issues.csv"),
        sorted.inspect(|issue| top.offer(issue)),
    )?;
    crate::memtrace::mem_trace("post_write_issues_csv");
    write_report_html(
        &out.join("report.html"),
        &summary,
        &top.into_sorted(),
        &sources,
    )?;
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
        "Scanned {} records across {} file(s). {} issues ({} critical, {} high). Score: {:.1}/100.",
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

    let mut records: Vec<EmirRecord> = Vec::new();
    for path in &inputs {
        if has_extension(path, "xml") {
            let mut outcome = read_emir_xml(path)?;
            info!(
                file = %path.display(),
                records = outcome.records.len(),
                "loaded EMIR XML for normalization",
            );
            records.append(&mut outcome.records);
        } else if has_extension(path, "csv") {
            let mapping = csv_mapping
                .as_ref()
                .expect("csv_mapping is Some when at least one CSV is in the input set");
            let mut csv_records = read_emir_csv(path, mapping)?;
            info!(
                file = %path.display(),
                records = csv_records.len(),
                "loaded EMIR CSV for normalization",
            );
            records.append(&mut csv_records);
        } else {
            warn!(path = %path.display(), "unsupported file extension; only XML and CSV are normalized");
        }
    }

    write_emir_parquet(out, &records)
        .with_context(|| format!("writing Parquet to {}", out.display()))?;

    let bytes = std::fs::metadata(out).map(|m| m.len()).unwrap_or(0);
    println!(
        "Normalized {} EMIR record(s) into {} ({} bytes, Snappy-compressed Parquet).",
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
            "opendqi emir validate only supports XML files; pass an XML file or a directory containing XML files"
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
                // Skip XSD validation if XML is malformed; xmllint
                // would just report the same parse error.
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
    let outcome = read_emir_feedback_xml(input)
        .with_context(|| format!("reading feedback file {}", input.display()))?;
    info!(
        file = %input.display(),
        records = outcome.records.len(),
        format_issues = outcome.issues.len(),
        "loaded EMIR feedback XML",
    );

    let mut store = opendqi_store::open_store(store_path)
        .with_context(|| format!("opening history store at {}", store_path.display()))?;

    // Persist the feedback batch into the `feedbacks` table so the
    // `opendqi feedback list/resolve/stale` workflow can pick it up.
    let persisted = store
        .persist_feedback_batch(&outcome.records)
        .context("persisting EMIR feedback batch to history store")?;
    info!(persisted, "feedback rows persisted to store");

    let utis: Vec<&str> = outcome
        .records
        .iter()
        .filter_map(|r| r.uti.as_deref())
        .map(str::trim)
        .filter(|u| !u.is_empty())
        .collect();
    let prior = store
        .load_prior_emir(&utis, i64::MAX)
        .context("loading prior EMIR records from history store")?;
    info!(prior_records = prior.len(), "loaded prior records");

    let now = Utc::now();
    let ctx = CheckContext {
        thresholds: Thresholds::default(),
        today: now.date_naive(),
        now,
    };
    // Streaming pipeline (Milestone 0.25): every issue source flows
    // into one spill-capable sink instead of a union `Vec<DqIssue>`.
    // `records_processed` = number of feedback lines parsed (the
    // "throughput" indicator), exactly as the former
    // `build_feedback_summary`; small inputs never spill ⇒ output
    // byte-identical.
    let inputs = [input.to_path_buf()];
    let sources = vec![input.to_string_lossy().into_owned()];
    let sink = std::sync::Mutex::new(SortedIssueSink::new(
        &ctx.thresholds,
        STREAM_SPILL_MAX_ISSUES,
    ));
    sink.lock()
        .expect("issue sink mutex")
        .push_batch(outcome.issues);
    stream_checks_into(&default_feedback_checks(), &sink, |c| {
        c.run(&outcome.records, &prior, &ctx)
    });
    let (summary, sorted) = sink.into_inner().expect("issue sink mutex").finish(
        Regime::Emir,
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
            info!(to = ?cfg.to, "feedback report emailed");
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
        "Ingested {} feedback record(s). {} issues ({} critical, {} high). Score: {:.1}/100.",
        summary.records_processed, summary.issues_total, critical, high, summary.quality_score
    );
    println!("Report: {}", out.join("report.html").display());
    Ok(())
}

fn run_recon_stats(
    input: &Path,
    prior_path: Option<&Path>,
    config_path: Option<&Path>,
    out: &Path,
    email_config_path: Option<&Path>,
) -> Result<()> {
    let started_at = Utc::now();

    let thresholds = match config_path {
        Some(p) => {
            let s = std::fs::read_to_string(p)
                .with_context(|| format!("reading config {}", p.display()))?;
            serde_yaml::from_str(&s).with_context(|| format!("parsing config {}", p.display()))?
        }
        None => Thresholds::default(),
    };

    let outcome = read_emir_recon_stats_xml(input)
        .with_context(|| format!("reading auth.091 file {}", input.display()))?;
    info!(
        file = %input.display(),
        records = outcome.records.len(),
        format_issues = outcome.issues.len(),
        "loaded EMIR auth.091 XML",
    );

    let prior: Vec<opendqi_core::ReconStatsRecord> = if let Some(p) = prior_path {
        let prior_outcome = read_emir_recon_stats_xml(p)
            .with_context(|| format!("reading prior auth.091 file {}", p.display()))?;
        info!(
            records = prior_outcome.records.len(),
            "loaded prior auth.091"
        );
        prior_outcome.records
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let ctx = CheckContext {
        thresholds,
        today: now.date_naive(),
        now,
    };

    // Streaming pipeline (Milestone 0.25): one spill-capable sink for
    // every issue source; `records_processed` = parsed auth.091 record
    // count (exactly as the former `build_feedback_summary`).
    // Byte-identical for non-spilling inputs.
    let inputs = [input.to_path_buf()];
    let sources = vec![input.to_string_lossy().into_owned()];
    let sink = std::sync::Mutex::new(SortedIssueSink::new(
        &ctx.thresholds,
        STREAM_SPILL_MAX_ISSUES,
    ));
    sink.lock()
        .expect("issue sink mutex")
        .push_batch(outcome.issues);
    stream_checks_into(&default_recon_stats_checks(), &sink, |c| {
        c.run(&outcome.records, &prior, &ctx)
    });
    // Per-transaction RcncltnRpt detail (UTI, cohort-inherited
    // pairing/recon status, mismatched criterion names) → EMIR.REC.*.
    stream_checks_into(&default_reconciliation_checks(), &sink, |c| {
        c.run(&outcome.reconciliation_records, &[], &ctx)
    });
    let (summary, sorted) = sink.into_inner().expect("issue sink mutex").finish(
        Regime::Emir,
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
        &out.join("recon_stats_issues.csv"),
        sorted.inspect(|issue| top.offer(issue)),
    )?;
    let top = top.into_sorted();
    write_report_html(
        &out.join("recon_stats_report.html"),
        &summary,
        &top,
        &sources,
    )?;

    if let Some(path) = email_config_path {
        let cfg = opendqi_report::SmtpConfig::from_yaml_file(path)?;
        let sent = opendqi_report::send_report_email(
            &cfg,
            &summary,
            &out.join("recon_stats_report.html"),
            &out.join("summary.json"),
            &out.join("recon_stats_issues.csv"),
        )?;
        if sent {
            info!(to = ?cfg.to, "recon-stats report emailed");
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
        "Scanned {} recon-stats record(s). {} issues ({} critical, {} high). Score: {:.1}/100.",
        summary.records_processed, summary.issues_total, critical, high, summary.quality_score
    );
    println!("Report: {}", out.join("recon_stats_report.html").display());
    Ok(())
}

fn run_warnings(
    input: &Path,
    config_path: Option<&Path>,
    out: &Path,
    email_config_path: Option<&Path>,
) -> Result<()> {
    let started_at = Utc::now();

    let thresholds = match config_path {
        Some(p) => {
            let s = std::fs::read_to_string(p)
                .with_context(|| format!("reading config {}", p.display()))?;
            serde_yaml::from_str(&s).with_context(|| format!("parsing config {}", p.display()))?
        }
        None => Thresholds::default(),
    };

    let outcome = read_emir_warnings_xml(input)
        .with_context(|| format!("reading auth.106 file {}", input.display()))?;
    info!(
        file = %input.display(),
        records = outcome.records.len(),
        format_issues = outcome.issues.len(),
        "loaded EMIR auth.106 XML",
    );

    let now = Utc::now();
    let ctx = CheckContext {
        thresholds,
        today: now.date_naive(),
        now,
    };

    // Streaming pipeline (Milestone 0.25): report-level, per-counterparty
    // and per-UTI warnings all fold into one spill-capable sink;
    // `records_processed` = parsed auth.106 record count (as the former
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
    stream_checks_into(&default_warnings_checks(), &sink, |c| {
        c.run(&outcome.records, &ctx)
    });
    // Fold the per-counterparty `Wrnngs` issues into the same report
    // (shared core; same precedent as recon-stats' per-tx fold).
    stream_checks_into(&default_warnings_counterparty_checks(), &sink, |c| {
        c.run(&outcome.counterparty_records, &ctx)
    });
    // Per-UTI `Wrnngs/TxDtls` issues — same report, same CSV.
    stream_checks_into(&default_warnings_transaction_checks(), &sink, |c| {
        c.run(&outcome.transaction_records, &ctx)
    });
    let (summary, sorted) = sink.into_inner().expect("issue sink mutex").finish(
        Regime::Emir,
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
        &out.join("warnings_issues.csv"),
        sorted.inspect(|issue| top.offer(issue)),
    )?;
    let top = top.into_sorted();
    write_report_html(&out.join("warnings_report.html"), &summary, &top, &sources)?;

    if let Some(path) = email_config_path {
        let cfg = opendqi_report::SmtpConfig::from_yaml_file(path)?;
        let sent = opendqi_report::send_report_email(
            &cfg,
            &summary,
            &out.join("warnings_report.html"),
            &out.join("summary.json"),
            &out.join("warnings_issues.csv"),
        )?;
        if sent {
            info!(to = ?cfg.to, "warnings report emailed");
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
        "Scanned {} warnings record(s). {} issues ({} critical, {} high). Score: {:.1}/100.",
        summary.records_processed, summary.issues_total, critical, high, summary.quality_score
    );
    println!("Report: {}", out.join("warnings_report.html").display());
    Ok(())
}

fn run_tr_state_scan(
    input: &Path,
    store_path: Option<&Path>,
    out: &Path,
    email_config_path: Option<&Path>,
) -> Result<()> {
    let started_at = Utc::now();
    let outcome = read_emir_tr_state_xml(input)
        .with_context(|| format!("reading TSR file {}", input.display()))?;
    info!(
        file = %input.display(),
        records = outcome.records.len(),
        format_issues = outcome.issues.len(),
        "loaded EMIR TSR XML",
    );

    // Optional store enrichment: persist this TSR snapshot, load
    // prior EMIR records for the UTIs present in the TSR, and run
    // the cross-batch lifecycle layer against prior TSR snapshots.
    let (prior, prior_tsr): (Vec<EmirRecord>, Vec<TrStateRecord>) =
        if let Some(store_path) = store_path {
            let mut store = opendqi_store::open_store(store_path)
                .with_context(|| format!("opening history store at {}", store_path.display()))?;
            let scan_id = store
                .persist_tr_state_batch(1, &outcome.records)
                .context("persisting EMIR TSR batch to history store")?;
            let utis: Vec<&str> = outcome
                .records
                .iter()
                .filter_map(|r| r.uti.as_deref())
                .map(str::trim)
                .filter(|u| !u.is_empty())
                .collect();
            let prior = store
                .load_prior_emir(&utis, i64::MAX)
                .context("loading prior EMIR records from history store")?;
            let prior_tsr = store
                .load_latest_prior_tr_state(scan_id)
                .context("loading prior EMIR TSR snapshot from history store")?;
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
    // Streaming pipeline (Milestone 0.25): TSR + conditional lifecycle
    // fold into one spill-capable sink; `records_processed` = parsed
    // TSR record count (as the former `build_feedback_summary`).
    // Byte-identical for non-spilling inputs.
    let inputs = [input.to_path_buf()];
    let sources = vec![input.to_string_lossy().into_owned()];
    let sink = std::sync::Mutex::new(SortedIssueSink::new(
        &ctx.thresholds,
        STREAM_SPILL_MAX_ISSUES,
    ));
    sink.lock()
        .expect("issue sink mutex")
        .push_batch(outcome.issues);
    stream_checks_into(&default_tr_state_checks(), &sink, |c| {
        c.run(&outcome.records, &prior, &ctx)
    });
    if !prior_tsr.is_empty() {
        stream_checks_into(&default_tr_state_lifecycle_checks(), &sink, |c| {
            c.run(&outcome.records, &prior_tsr, &ctx)
        });
    }
    let (summary, sorted) = sink.into_inner().expect("issue sink mutex").finish(
        Regime::Emir,
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
            info!(to = ?cfg.to, "TSR report emailed");
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
        "Scanned {} TSR record(s). {} issues ({} critical, {} high). Score: {:.1}/100.",
        summary.records_processed, summary.issues_total, critical, high, summary.quality_score
    );
    println!("Report: {}", out.join("tr_state_report.html").display());
    Ok(())
}

fn run_mar_scan(
    input: &Path,
    store_path: Option<&Path>,
    out: &Path,
    email_config_path: Option<&Path>,
) -> Result<()> {
    let started_at = Utc::now();
    let outcome = read_emir_mar_xml(input)
        .with_context(|| format!("reading MAR file {}", input.display()))?;
    info!(
        file = %input.display(),
        records = outcome.records.len(),
        format_issues = outcome.issues.len(),
        "loaded EMIR MAR XML",
    );

    let prior: Vec<MarginActivityRecord> = if let Some(store_path) = store_path {
        let mut store = opendqi_store::open_store(store_path)
            .with_context(|| format!("opening history store at {}", store_path.display()))?;
        let scan_id = store
            .persist_margin_activity_batch(1, &outcome.records)
            .context("persisting MAR batch to history store")?;
        // Use the latest-prior loader so PORTFOLIO_GAP can see
        // portfolios that disappeared from the current batch.
        let prior = store
            .load_latest_prior_margin_activity(scan_id)
            .context("loading prior MAR snapshot from history store")?;
        info!(prior_records = prior.len(), "loaded prior MAR records");
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
    // Streaming pipeline (Milestone 0.25): MAR + conditional lifecycle
    // fold into one spill-capable sink; `records_processed` = parsed
    // MAR record count (as the former `build_feedback_summary`).
    // Byte-identical for non-spilling inputs.
    let inputs = [input.to_path_buf()];
    let sources = vec![input.to_string_lossy().into_owned()];
    let sink = std::sync::Mutex::new(SortedIssueSink::new(
        &ctx.thresholds,
        STREAM_SPILL_MAX_ISSUES,
    ));
    sink.lock()
        .expect("issue sink mutex")
        .push_batch(outcome.issues);
    stream_checks_into(&default_margin_activity_checks(), &sink, |c| {
        c.run(&outcome.records, &prior, &ctx)
    });
    if !prior.is_empty() {
        stream_checks_into(&default_margin_activity_lifecycle_checks(), &sink, |c| {
            c.run(&outcome.records, &prior, &ctx)
        });
    }
    let (summary, sorted) = sink.into_inner().expect("issue sink mutex").finish(
        Regime::Emir,
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
        &out.join("mar_issues.csv"),
        sorted.inspect(|issue| top.offer(issue)),
    )?;
    let top = top.into_sorted();
    write_report_html(&out.join("mar_report.html"), &summary, &top, &sources)?;

    if let Some(path) = email_config_path {
        let cfg = opendqi_report::SmtpConfig::from_yaml_file(path)?;
        let sent = opendqi_report::send_report_email(
            &cfg,
            &summary,
            &out.join("mar_report.html"),
            &out.join("summary.json"),
            &out.join("mar_issues.csv"),
        )?;
        if sent {
            info!(to = ?cfg.to, "MAR report emailed");
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
        "Scanned {} MAR record(s). {} issues ({} critical, {} high). Score: {:.1}/100.",
        summary.records_processed, summary.issues_total, critical, high, summary.quality_score
    );
    println!("Report: {}", out.join("mar_report.html").display());
    Ok(())
}

fn run_msr_scan(
    input: &Path,
    store_path: Option<&Path>,
    out: &Path,
    email_config_path: Option<&Path>,
) -> Result<()> {
    let started_at = Utc::now();
    let outcome = read_emir_msr_xml(input)
        .with_context(|| format!("reading MSR file {}", input.display()))?;
    info!(
        file = %input.display(),
        records = outcome.records.len(),
        format_issues = outcome.issues.len(),
        "loaded EMIR MSR XML",
    );

    let (prior, prior_msr): (Vec<EmirRecord>, Vec<MarginStateRecord>) =
        if let Some(store_path) = store_path {
            let mut store = opendqi_store::open_store(store_path)
                .with_context(|| format!("opening history store at {}", store_path.display()))?;
            let scan_id = store
                .persist_margin_state_batch(1, &outcome.records)
                .context("persisting EMIR MSR batch to history store")?;
            let utis: Vec<&str> = outcome
                .records
                .iter()
                .filter_map(|r| r.uti.as_deref())
                .map(str::trim)
                .filter(|u| !u.is_empty())
                .collect();
            let prior = store
                .load_prior_emir(&utis, i64::MAX)
                .context("loading prior EMIR records from history store")?;
            let prior_msr = store
                .load_latest_prior_margin_state(scan_id)
                .context("loading prior MSR snapshot from history store")?;
            info!(
                prior_records = prior.len(),
                prior_msr_records = prior_msr.len(),
                "loaded prior records"
            );
            (prior, prior_msr)
        } else {
            (Vec::new(), Vec::new())
        };

    let now = Utc::now();
    let ctx = CheckContext {
        thresholds: Thresholds::default(),
        today: now.date_naive(),
        now,
    };
    // Streaming pipeline (Milestone 0.25): MSR + conditional lifecycle
    // fold into one spill-capable sink; `records_processed` = parsed
    // MSR record count (as the former `build_feedback_summary`).
    // Byte-identical for non-spilling inputs.
    let inputs = [input.to_path_buf()];
    let sources = vec![input.to_string_lossy().into_owned()];
    let sink = std::sync::Mutex::new(SortedIssueSink::new(
        &ctx.thresholds,
        STREAM_SPILL_MAX_ISSUES,
    ));
    sink.lock()
        .expect("issue sink mutex")
        .push_batch(outcome.issues);
    stream_checks_into(&default_margin_state_checks(), &sink, |c| {
        c.run(&outcome.records, &prior, &ctx)
    });
    if !prior_msr.is_empty() {
        stream_checks_into(&default_margin_state_lifecycle_checks(), &sink, |c| {
            c.run(&outcome.records, &prior_msr, &ctx)
        });
    }
    let (summary, sorted) = sink.into_inner().expect("issue sink mutex").finish(
        Regime::Emir,
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
        &out.join("msr_issues.csv"),
        sorted.inspect(|issue| top.offer(issue)),
    )?;
    let top = top.into_sorted();
    write_report_html(&out.join("msr_report.html"), &summary, &top, &sources)?;

    if let Some(path) = email_config_path {
        let cfg = opendqi_report::SmtpConfig::from_yaml_file(path)?;
        let sent = opendqi_report::send_report_email(
            &cfg,
            &summary,
            &out.join("msr_report.html"),
            &out.join("summary.json"),
            &out.join("msr_issues.csv"),
        )?;
        if sent {
            info!(to = ?cfg.to, "MSR report emailed");
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
        "Scanned {} MSR record(s). {} issues ({} critical, {} high). Score: {:.1}/100.",
        summary.records_processed, summary.issues_total, critical, high, summary.quality_score
    );
    println!("Report: {}", out.join("msr_report.html").display());
    Ok(())
}

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
    info!(count = inputs.len(), "discovered TAR inputs");

    let mut records: Vec<EmirRecord> = Vec::new();
    let mut format_issues: Vec<DqIssue> = Vec::new();
    let mut sources: Vec<String> = Vec::with_capacity(inputs.len());

    for path in &inputs {
        sources.push(path.to_string_lossy().into_owned());
        if has_extension(path, "xml") {
            let mut outcome = read_emir_xml(path)?;
            info!(
                file = %path.display(),
                trades = outcome.records.len(),
                "loaded TAR XML",
            );
            records.append(&mut outcome.records);
            format_issues.append(&mut outcome.issues);
        } else {
            warn!(path = %path.display(), "unsupported file extension; only XML supported by tr-activity-scan");
        }
    }

    // Optional store enrichment (prior submission history).
    let prior: Vec<EmirRecord> = if let Some(store_path) = store_path {
        let store = opendqi_store::open_store(store_path)
            .with_context(|| format!("opening history store at {}", store_path.display()))?;
        let utis: Vec<&str> = records
            .iter()
            .filter_map(|r| r.uti.as_deref())
            .map(str::trim)
            .filter(|u| !u.is_empty())
            .collect();
        let prior = store
            .load_prior_emir(&utis, i64::MAX)
            .context("loading prior EMIR records from history store")?;
        info!(prior_records = prior.len(), "loaded prior records");
        prior
    } else {
        Vec::new()
    };

    // Optional companion TSR for TAR↔TSR coherence.
    let tsr_records = if let Some(tsr_path) = tsr_path {
        let outcome = opendqi_xml::read_emir_tr_state_xml(tsr_path)
            .with_context(|| format!("reading companion TSR {}", tsr_path.display()))?;
        info!(records = outcome.records.len(), "loaded companion TSR");
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

    // Activity distributions.
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

    // Streaming pipeline (Milestone 0.25): format + TAR issues fold
    // into one spill-capable sink; `records_processed` = records.len()
    // (Pattern A, exactly as the former `build_summary`). The activity
    // distribution sibling JSON is record-derived and unchanged.
    // Byte-identical for non-spilling inputs.
    let sink = std::sync::Mutex::new(SortedIssueSink::new(
        &ctx.thresholds,
        STREAM_SPILL_MAX_ISSUES,
    ));
    sink.lock()
        .expect("issue sink mutex")
        .push_batch(format_issues);
    stream_checks_into(&default_tr_activity_checks(), &sink, |c| {
        c.run(&records, &prior, tsr_records.as_deref(), &ctx)
    });
    let (summary, sorted) = sink.into_inner().expect("issue sink mutex").finish(
        Regime::Emir,
        inputs.len() as u32,
        records.len() as u32,
        started_at,
        Utc::now(),
    );

    std::fs::create_dir_all(out)
        .with_context(|| format!("creating output directory {}", out.display()))?;
    write_summary_json(&out.join("summary.json"), &summary)?;
    // Activity-specific summary embedding distributions (sibling
    // file so the standard `summary.json` remains regime-uniform).
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
            info!(to = ?cfg.to, "TAR report emailed");
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
        "Scanned {} TAR record(s). {} issues ({} critical, {} high). Score: {:.1}/100.",
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
    let started_at = Utc::now();

    // 1. Load all three layers.
    let tar_inputs = discover_emir_inputs(tar_path)?;
    let mut tar_records: Vec<EmirRecord> = Vec::new();
    let mut tar_issues: Vec<DqIssue> = Vec::new();
    for p in &tar_inputs {
        if !has_extension(p, "xml") {
            warn!(path = %p.display(), "skipping non-XML file in TAR input");
            continue;
        }
        let mut outcome = read_emir_xml(p)?;
        tar_records.append(&mut outcome.records);
        tar_issues.append(&mut outcome.issues);
    }
    info!(records = tar_records.len(), "loaded TAR");

    let tsr_outcome = read_emir_tr_state_xml(tsr_path)
        .with_context(|| format!("reading TSR {}", tsr_path.display()))?;
    info!(records = tsr_outcome.records.len(), "loaded TSR");

    let feedback_outcome = read_emir_feedback_xml(feedback_path)
        .with_context(|| format!("reading feedback {}", feedback_path.display()))?;
    info!(records = feedback_outcome.records.len(), "loaded feedback");

    // 2. Optional store enrichment.
    let prior: Vec<EmirRecord> = if let Some(sp) = store_path {
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
            .load_prior_emir(&utis, i64::MAX)
            .context("loading prior EMIR records from history store")?;
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

    // 3. Streaming pipeline (Milestone 0.25): every layer's issues
    //    fold into one spill-capable sink (Pattern A:
    //    `records_processed` = tar_records.len(), exactly as the
    //    former `build_summary`). Byte-identical for non-spilling
    //    inputs.
    let all_inputs = [
        tar_path.to_path_buf(),
        tsr_path.to_path_buf(),
        feedback_path.to_path_buf(),
    ];
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
        s.push_batch(feedback_outcome.issues.clone());
    }
    stream_checks_into(&default_checks(), &sink, |c| c.run(&tar_records, &ctx));
    stream_checks_into(&default_lifecycle_checks(), &sink, |c| {
        c.run(&tar_records, &prior, &ctx)
    });
    stream_checks_into(&default_tr_state_checks(), &sink, |c| {
        c.run(&tsr_outcome.records, &prior, &ctx)
    });
    stream_checks_into(&default_feedback_checks(), &sink, |c| {
        c.run(&feedback_outcome.records, &prior, &ctx)
    });
    stream_checks_into(&default_tr_activity_checks(), &sink, |c| {
        c.run(&tar_records, &prior, Some(&tsr_outcome.records), &ctx)
    });
    // 4. Cross-layer coherence checks (EMIR.AUD.*).
    sink.lock().expect("issue sink mutex").push_batch(
        opendqi_core::dq::compute_tr_audit_emir_issues(
            &tar_records,
            &tsr_outcome.records,
            &feedback_outcome.records,
            &tsr_path.to_string_lossy(),
            &feedback_path.to_string_lossy(),
        ),
    );

    let (summary, sorted) = sink.into_inner().expect("issue sink mutex").finish(
        Regime::Emir,
        all_inputs.len() as u32,
        tar_records.len() as u32,
        started_at,
        Utc::now(),
    );

    // 5. Write outputs.
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
            info!(to = ?cfg.to, "TR audit report emailed");
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
        "TR audit: TAR={} TSR={} feedback={}. {} issues ({} critical, {} high). Score: {:.1}/100.",
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

fn run_collateral_audit(
    tsr_path: &Path,
    msr_path: &Path,
    _store_path: Option<&Path>,
    config_path: Option<&Path>,
    out: &Path,
    email_config_path: Option<&Path>,
) -> Result<()> {
    let started_at = Utc::now();

    // 1. Load thresholds (optional YAML; defaults otherwise).
    let thresholds = match config_path {
        Some(p) => {
            let s = std::fs::read_to_string(p)
                .with_context(|| format!("reading config {}", p.display()))?;
            serde_yaml::from_str(&s).with_context(|| format!("parsing config {}", p.display()))?
        }
        None => Thresholds::default(),
    };

    // 2. Load both messages.
    let tsr_outcome = read_emir_tr_state_xml(tsr_path)
        .with_context(|| format!("reading TSR {}", tsr_path.display()))?;
    info!(records = tsr_outcome.records.len(), "loaded TSR");
    let msr_outcome = read_emir_msr_xml(msr_path)
        .with_context(|| format!("reading MSR {}", msr_path.display()))?;
    info!(records = msr_outcome.records.len(), "loaded MSR");

    // 3. Streaming pipeline (post-Increment D rebase): format issues
    //    from both parses + the cross-layer collateral compute
    //    (`EMIR.COL.MISSING` / `EMIR.COL.STALE`) fold into one
    //    spill-capable sink. Mirrors the M0.25 pattern used by every
    //    other EMIR command on main. `records_processed` = parsed
    //    TSR record count (the cross-ref's "outstanding" universe).
    //    Byte-equivalent (no-spill == finalize, M0.22-locked invariant).
    let now = Utc::now();
    let ctx = CheckContext {
        thresholds,
        today: now.date_naive(),
        now,
    };
    let all_inputs = [tsr_path.to_path_buf(), msr_path.to_path_buf()];
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
        s.push_batch(tsr_outcome.issues.clone());
        s.push_batch(msr_outcome.issues.clone());
        s.push_batch(opendqi_core::dq::compute_collateral_emir_issues(
            &tsr_outcome.records,
            &msr_outcome.records,
            &ctx.thresholds,
            now,
        ));
    }
    let (summary, sorted) = sink.into_inner().expect("issue sink mutex").finish(
        Regime::Emir,
        all_inputs.len() as u32,
        tsr_outcome.records.len() as u32,
        started_at,
        Utc::now(),
    );

    std::fs::create_dir_all(out)
        .with_context(|| format!("creating output directory {}", out.display()))?;
    write_summary_json(&out.join("summary.json"), &summary)?;
    let mut top = TopIssues::with_capacity(20);
    write_issues_csv_from_iter(
        &out.join("collateral_audit_issues.csv"),
        sorted.inspect(|issue| top.offer(issue)),
    )?;
    let top = top.into_sorted();
    write_report_html(
        &out.join("collateral_audit_report.html"),
        &summary,
        &top,
        &sources,
    )?;

    if let Some(path) = email_config_path {
        let cfg = opendqi_report::SmtpConfig::from_yaml_file(path)?;
        let sent = opendqi_report::send_report_email(
            &cfg,
            &summary,
            &out.join("collateral_audit_report.html"),
            &out.join("summary.json"),
            &out.join("collateral_audit_issues.csv"),
        )?;
        if sent {
            info!(to = ?cfg.to, "collateral audit report emailed");
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
        "Cross-checked {} TSR record(s) against {} MSR row(s). {} issues ({} critical, {} high). Score: {:.1}/100.",
        summary.records_processed,
        msr_outcome.records.len(),
        summary.issues_total,
        critical,
        high,
        summary.quality_score
    );
    println!(
        "Report: {}",
        out.join("collateral_audit_report.html").display()
    );
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
    let book_records = read_emir_csv(book_path, &mapping)
        .with_context(|| format!("reading book CSV {}", book_path.display()))?;
    info!(records = book_records.len(), "loaded book");

    let tsr_outcome = read_emir_tr_state_xml(tsr_path)
        .with_context(|| format!("reading TSR {}", tsr_path.display()))?;
    info!(records = tsr_outcome.records.len(), "loaded TSR");

    // Streaming pipeline (Milestone 0.25): book-vs-TSR issues fold
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
        s.push_batch(compute_book_reconcile_issues(
            &book_records,
            &tsr_outcome.records,
        ));
    }
    let (summary, sorted) = sink.into_inner().expect("issue sink mutex").finish(
        Regime::Emir,
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
            info!(to = ?cfg.to, "book-vs-TSR report emailed");
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
        "Book vs TSR: book={} tsr={}. {} issues ({} critical, {} high). Score: {:.1}/100.",
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
mod book_reconcile_tests {
    use super::*;
    use chrono::NaiveDate;
    use opendqi_core::TrStateRecord;
    use rust_decimal::Decimal;

    fn book_baseline(uti: &str) -> EmirRecord {
        EmirRecord {
            uti: Some(uti.into()),
            notional_amount: Some(Decimal::from(1_000_000)),
            notional_currency: Some("EUR".into()),
            valuation_amount: Some(Decimal::new(15050, 2)),
            valuation_currency: Some("EUR".into()),
            maturity_date: NaiveDate::from_ymd_opt(2031, 4, 2),
            ..Default::default()
        }
    }

    fn tsr_baseline(uti: &str) -> TrStateRecord {
        TrStateRecord {
            uti: Some(uti.into()),
            notional_amount: Some(Decimal::from(1_000_000)),
            notional_currency: Some("EUR".into()),
            valuation_amount: Some(Decimal::new(15050, 2)),
            valuation_currency: Some("EUR".into()),
            maturity_date: NaiveDate::from_ymd_opt(2031, 4, 2),
            status: Some("OUTSTANDING".into()),
            ..Default::default()
        }
    }

    #[test]
    fn baseline_match_emits_nothing() {
        let book = vec![book_baseline("U1")];
        let tsr = vec![tsr_baseline("U1")];
        let issues = compute_book_reconcile_issues(&book, &tsr);
        assert!(issues.is_empty(), "expected no issues, got {issues:?}");
    }

    #[test]
    fn in_book_not_in_tsr() {
        let book = vec![book_baseline("U-ONLY-BOOK")];
        let tsr = vec![];
        let issues = compute_book_reconcile_issues(&book, &tsr);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].check_id, "EMIR.BREC.IN_BOOK_NOT_IN_TSR");
    }

    #[test]
    fn in_tsr_not_in_book_fires_only_when_outstanding() {
        let book = vec![];
        let mut t = tsr_baseline("U-ONLY-TSR");
        let outstanding = vec![t.clone()];
        let issues = compute_book_reconcile_issues(&book, &outstanding);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].check_id, "EMIR.BREC.IN_TSR_NOT_IN_BOOK");
        // Now mark as TERMINATED → should NOT fire.
        t.status = Some("TERMINATED".into());
        let terminated = vec![t];
        let issues = compute_book_reconcile_issues(&book, &terminated);
        assert!(issues.is_empty());
    }

    #[test]
    fn notional_mismatch() {
        let mut b = book_baseline("U1");
        b.notional_amount = Some(Decimal::from(9_999_999));
        let issues = compute_book_reconcile_issues(&[b], &[tsr_baseline("U1")]);
        assert!(issues
            .iter()
            .any(|i| i.check_id == "EMIR.BREC.NOTIONAL_MISMATCH"));
    }

    #[test]
    fn notional_currency_mismatch() {
        let mut b = book_baseline("U1");
        b.notional_currency = Some("USD".into());
        let issues = compute_book_reconcile_issues(&[b], &[tsr_baseline("U1")]);
        assert!(issues
            .iter()
            .any(|i| i.check_id == "EMIR.BREC.NOTIONAL_CURRENCY_MISMATCH"));
    }

    #[test]
    fn valuation_mismatch_outside_tolerance() {
        let mut b = book_baseline("U1");
        // 50% difference — well beyond 1% tolerance.
        b.valuation_amount = Some(Decimal::new(22575, 2));
        let issues = compute_book_reconcile_issues(&[b], &[tsr_baseline("U1")]);
        assert!(issues
            .iter()
            .any(|i| i.check_id == "EMIR.BREC.VALUATION_MISMATCH"));
    }

    #[test]
    fn valuation_within_tolerance_is_clean() {
        let mut b = book_baseline("U1");
        // 0.5% off — within 1% tolerance.
        b.valuation_amount = Some(Decimal::new(15125, 2));
        let issues = compute_book_reconcile_issues(&[b], &[tsr_baseline("U1")]);
        assert!(issues
            .iter()
            .all(|i| i.check_id != "EMIR.BREC.VALUATION_MISMATCH"));
    }

    #[test]
    fn maturity_mismatch() {
        let mut b = book_baseline("U1");
        b.maturity_date = NaiveDate::from_ymd_opt(2099, 12, 31);
        let issues = compute_book_reconcile_issues(&[b], &[tsr_baseline("U1")]);
        assert!(issues
            .iter()
            .any(|i| i.check_id == "EMIR.BREC.MATURITY_MISMATCH"));
    }

    #[test]
    fn status_mismatch_book_active_tsr_terminated() {
        let mut t = tsr_baseline("U1");
        t.status = Some("TERMINATED".into());
        let issues = compute_book_reconcile_issues(&[book_baseline("U1")], &[t]);
        assert!(issues
            .iter()
            .any(|i| i.check_id == "EMIR.BREC.STATUS_MISMATCH"));
    }
}

fn xsd_violation_issue(path: &Path, violation: &XsdViolation) -> DqIssue {
    let line_label = violation
        .line
        .map(|n| format!("line={n}"))
        .unwrap_or_default();
    DqIssue {
        check_id: CHECK_XSD_VIOLATION.into(),
        regime: Regime::Emir,
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
        regime: Regime::Emir,
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
#[allow(clippy::too_many_arguments)]
fn run_data_quality_pack(
    tsr_path: Option<&Path>,
    tar_path: Option<&Path>,
    msr_path: Option<&Path>,
    mar_path: Option<&Path>,
    feedback_path: Option<&Path>,
    positions_path: Option<&Path>,
    recon_stats_path: Option<&Path>,
    config_path: Option<&Path>,
    as_of_str: Option<&str>,
    out: &Path,
    email_config_path: Option<&Path>,
) -> Result<()> {
    if tsr_path.is_none()
        && tar_path.is_none()
        && msr_path.is_none()
        && mar_path.is_none()
        && feedback_path.is_none()
        && positions_path.is_none()
        && recon_stats_path.is_none()
    {
        return Err(anyhow!(
            "data-quality-pack: at least one of --tsr / --tar / --msr / --mar / \
             --feedback / --positions / --recon-stats is required"
        ));
    }

    // Resolve thresholds (default + optional YAML override).
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

    // Load each provided layer (XML readers; CSV mapping for the
    // DQI pack is deferred to a future increment).
    let tsr_records = match tsr_path {
        Some(p) => {
            let outcome = read_emir_tr_state_xml(p)
                .with_context(|| format!("reading TSR {}", p.display()))?;
            info!(records = outcome.records.len(), "loaded TSR");
            outcome.records
        }
        None => Vec::new(),
    };
    let mut tar_records: Vec<EmirRecord> = Vec::new();
    if let Some(p) = tar_path {
        let inputs = discover_emir_inputs(p)?;
        for q in &inputs {
            if !has_extension(q, "xml") {
                warn!(path = %q.display(), "skipping non-XML file in TAR input");
                continue;
            }
            let mut outcome = read_emir_xml(q)?;
            tar_records.append(&mut outcome.records);
        }
        info!(records = tar_records.len(), "loaded TAR");
    }
    let msr_records = match msr_path {
        Some(p) => {
            let outcome =
                read_emir_msr_xml(p).with_context(|| format!("reading MSR {}", p.display()))?;
            info!(records = outcome.records.len(), "loaded MSR");
            outcome.records
        }
        None => Vec::new(),
    };
    let mar_records = match mar_path {
        Some(p) => {
            let outcome =
                read_emir_mar_xml(p).with_context(|| format!("reading MAR {}", p.display()))?;
            info!(records = outcome.records.len(), "loaded MAR");
            outcome.records
        }
        None => Vec::new(),
    };
    let feedback_records = match feedback_path {
        Some(p) => {
            let outcome = read_emir_feedback_xml(p)
                .with_context(|| format!("reading feedback {}", p.display()))?;
            info!(records = outcome.records.len(), "loaded feedback");
            outcome.records
        }
        None => Vec::new(),
    };
    // v0.18 E1-E7: EMIR Position Set Report (auth.090) parsed,
    // then consumed by the 4 Position DQIs (E4) and 4 EMIR.POS.*
    // granular checks (E5).
    let position_records = match positions_path {
        Some(p) => {
            let outcome = read_emir_position_set_xml(p)
                .with_context(|| format!("reading EMIR positions {}", p.display()))?;
            info!(
                records = outcome.records.len(),
                "loaded EMIR positions (auth.090)"
            );
            outcome.records
        }
        None => Vec::new(),
    };
    // v0.18 F1: EMIR auth.091 Reconciliation Statistics — single
    // parse populates BOTH recon_stats (per-CP rates) AND
    // reconciliation (per-tx detail) slots. Activates 7 EMIR
    // cross-CP DQIs that have self-reported `not_applicable`
    // since v0.16 B1.
    let (recon_stats_records, reconciliation_records) = match recon_stats_path {
        Some(p) => {
            let outcome = read_emir_recon_stats_xml(p)
                .with_context(|| format!("reading EMIR auth.091 recon stats {}", p.display()))?;
            info!(
                stats = outcome.records.len(),
                reconciliations = outcome.reconciliation_records.len(),
                "loaded EMIR auth.091 (recon stats + per-tx reconciliation)"
            );
            (outcome.records, outcome.reconciliation_records)
        }
        None => (Vec::new(), Vec::new()),
    };

    // Detect raw_fields presence for the 2 gated indicators.
    // (Conservative: any record carries the key with a non-empty
    // value → on.)
    let has_conf = tar_records.iter().any(|r| {
        r.raw_fields
            .get("confirmation_timestamp")
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false)
    });
    let has_rec = tar_records.iter().any(|r| {
        r.raw_fields
            .get("reconciliation_status")
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false)
    });
    let mapping_presence = MappingPresence {
        has_confirmation_timestamp: has_conf,
        has_reconciliation_status: has_rec,
    };

    // Build the typed inputs view.
    let inputs = EmirDqiInputs {
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
        msr: if msr_path.is_some() {
            Some(&msr_records)
        } else {
            None
        },
        mar: if mar_path.is_some() {
            Some(&mar_records)
        } else {
            None
        },
        feedback: if feedback_path.is_some() {
            Some(&feedback_records)
        } else {
            None
        },
        // v0.18 F1 — recon_stats + reconciliation BOTH wired
        // through the single --recon-stats flag (one auth.091
        // file -> two slot vectors from the same parse).
        recon_stats: if recon_stats_path.is_some() {
            Some(&recon_stats_records)
        } else {
            None
        },
        reconciliation: if recon_stats_path.is_some() {
            Some(&reconciliation_records)
        } else {
            None
        },
        // v0.18 E7 — positions slot wired through --positions flag.
        positions: if positions_path.is_some() {
            Some(&position_records)
        } else {
            None
        },
    };

    let pack = compute_emir_dqi_pack(inputs, mapping_presence, &thresholds, as_of);

    // Collect source labels for the report context.
    let mut sources: Vec<String> = Vec::new();
    for (lbl, p) in [
        ("TSR", tsr_path),
        ("TAR", tar_path),
        ("MSR", msr_path),
        ("MAR", mar_path),
        ("Feedback", feedback_path),
        ("Positions", positions_path),
        ("ReconStats", recon_stats_path),
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
            info!(to = ?cfg.to, "data-quality-pack report emailed");
        } else {
            info!("email config is disabled — skipped send");
        }
    }

    // Friendly stdout summary.
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
        "Data Quality Pack: {}/{} indicators computed ({} red, {} amber). Granular: {} issues, score {:.1}/100.",
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
