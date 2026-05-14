//! `opendqi emir ...` subcommands.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use clap::Subcommand;
use opendqi_core::dq::{
    default_checks, default_feedback_checks, default_lifecycle_checks,
    default_margin_activity_checks, default_margin_activity_lifecycle_checks,
    default_margin_state_checks, default_margin_state_lifecycle_checks,
    default_reconciliation_checks, default_tr_activity_checks, default_tr_state_checks,
    default_tr_state_lifecycle_checks, finalize_issues, run_all, run_all_feedback,
    run_all_lifecycle, run_all_margin_activity, run_all_margin_state,
    run_all_margin_state_lifecycle, run_all_reconciliation, run_all_tr_activity, run_all_tr_state,
    run_all_tr_state_lifecycle, sort_issues, CheckContext,
};
use opendqi_core::{
    DqDimension, DqIssue, EmirRecord, MarginActivityRecord, MarginStateRecord, Regime, ScanSummary,
    Severity, Thresholds, TrActivitySummary, TrStateRecord,
};
use opendqi_io::{
    discover_emir_inputs, has_extension, read_emir_csv, read_emir_parquet, write_emir_parquet,
    CsvMapping,
};
use opendqi_report::{write_issues_csv, write_report_html, write_summary_json};
use opendqi_xml::{
    check_wellformedness, read_emir_feedback_xml, read_emir_mar_xml, read_emir_msr_xml,
    read_emir_reconciliation_xml, read_emir_tr_state_xml, read_emir_xml, ExternalXmllintValidator,
    XsdValidator, XsdViolation,
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
    },
    /// Ingest a TR pairing / matching report and produce
    /// `EMIR.REC.*` issues for UNPAIRED / UNRECONCILED trades and
    /// field-level mismatches.
    ///
    /// Naming caveat: this command reads a synthetic structure
    /// documented as `auth.106` v1; ESMA's official `auth.106` is
    /// a data-quality warning message and is on the Phase 3
    /// roadmap. See `docs/auth-messages.md`.
    Reconcile {
        /// Path to the `auth.106` XML file received from the TR.
        input: PathBuf,
        /// Required path to the SQLite history store.
        #[arg(long)]
        store: PathBuf,
        /// Directory where reports are written.
        #[arg(long)]
        out: PathBuf,
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
        } => {
            run_scan(
                &input,
                mapping.as_deref(),
                &out,
                config.as_deref(),
                xsd.as_deref(),
                store.as_deref(),
            )?;
            Ok(ExitCode::SUCCESS)
        }
        EmirAction::Validate { input, xsd } => run_validate(&input, &xsd),
        EmirAction::Feedback { input, store, out } => {
            run_feedback(&input, &store, &out)?;
            Ok(ExitCode::SUCCESS)
        }
        EmirAction::Reconcile { input, store, out } => {
            run_reconcile(&input, &store, &out)?;
            Ok(ExitCode::SUCCESS)
        }
        EmirAction::TrStateScan { input, store, out } => {
            run_tr_state_scan(&input, store.as_deref(), &out)?;
            Ok(ExitCode::SUCCESS)
        }
        EmirAction::TrActivityScan {
            input,
            store,
            tsr,
            out,
        } => {
            run_tr_activity_scan(&input, store.as_deref(), tsr.as_deref(), &out)?;
            Ok(ExitCode::SUCCESS)
        }
        EmirAction::TrAudit {
            tar,
            tsr,
            feedback,
            store,
            out,
        } => {
            run_tr_audit(&tar, &tsr, &feedback, store.as_deref(), &out)?;
            Ok(ExitCode::SUCCESS)
        }
        EmirAction::BookReconcile {
            book,
            tsr,
            mapping,
            out,
        } => {
            run_book_reconcile(&book, &tsr, &mapping, &out)?;
            Ok(ExitCode::SUCCESS)
        }
        EmirAction::MarScan { input, store, out } => {
            run_mar_scan(&input, store.as_deref(), &out)?;
            Ok(ExitCode::SUCCESS)
        }
        EmirAction::MsrScan { input, store, out } => {
            run_msr_scan(&input, store.as_deref(), &out)?;
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

fn run_scan(
    input: &Path,
    mapping_path: Option<&Path>,
    out: &Path,
    config_path: Option<&Path>,
    xsd_path: Option<&Path>,
    store_path: Option<&Path>,
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

    let now = Utc::now();
    let ctx = CheckContext {
        thresholds,
        today: now.date_naive(),
        now,
    };

    let checks = default_checks();
    let mut issues = run_all(&checks, &records, &ctx);
    issues.extend(format_issues);

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
        issues.extend(lifecycle_issues);
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

fn run_feedback(input: &Path, store_path: &Path, out: &Path) -> Result<()> {
    let started_at = Utc::now();
    let outcome = read_emir_feedback_xml(input)
        .with_context(|| format!("reading feedback file {}", input.display()))?;
    info!(
        file = %input.display(),
        records = outcome.records.len(),
        format_issues = outcome.issues.len(),
        "loaded EMIR feedback XML",
    );

    let mut issues: Vec<DqIssue> = outcome.issues;

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
    let fbk_issues = run_all_feedback(&default_feedback_checks(), &outcome.records, &prior, &ctx);
    info!(feedback_issues = fbk_issues.len(), "feedback checks run");
    issues.extend(fbk_issues);
    finalize_issues(&mut issues, &ctx);

    // Build a minimal summary. records_processed = number of feedback
    // lines parsed (acts as the "throughput" indicator).
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
        regime: Regime::Emir,
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

fn run_reconcile(input: &Path, store_path: &Path, out: &Path) -> Result<()> {
    let started_at = Utc::now();
    let outcome = read_emir_reconciliation_xml(input)
        .with_context(|| format!("reading reconciliation file {}", input.display()))?;
    info!(
        file = %input.display(),
        records = outcome.records.len(),
        format_issues = outcome.issues.len(),
        "loaded EMIR reconciliation XML",
    );

    let mut issues: Vec<DqIssue> = outcome.issues;

    let mut store = opendqi_store::open_store(store_path)
        .with_context(|| format!("opening history store at {}", store_path.display()))?;

    let persisted = store
        .persist_reconciliation_batch(&outcome.records)
        .context("persisting EMIR reconciliation batch to history store")?;
    info!(persisted, "reconciliation rows persisted to store");

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
    let rec_issues = run_all_reconciliation(
        &default_reconciliation_checks(),
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
        "Ingested {} reconciliation record(s). {} issues ({} critical, {} high). Score: {:.1}/100.",
        summary.records_processed, summary.issues_total, critical, high, summary.quality_score
    );
    println!("Report: {}", out.join("report.html").display());
    Ok(())
}

fn run_tr_state_scan(input: &Path, store_path: Option<&Path>, out: &Path) -> Result<()> {
    let started_at = Utc::now();
    let outcome = read_emir_tr_state_xml(input)
        .with_context(|| format!("reading TSR file {}", input.display()))?;
    info!(
        file = %input.display(),
        records = outcome.records.len(),
        format_issues = outcome.issues.len(),
        "loaded EMIR TSR XML",
    );

    let mut issues: Vec<DqIssue> = outcome.issues;

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
    let tsr_issues = run_all_tr_state(&default_tr_state_checks(), &outcome.records, &prior, &ctx);
    info!(tsr_issues = tsr_issues.len(), "TSR checks run");
    issues.extend(tsr_issues);
    if !prior_tsr.is_empty() {
        let lfc_issues = run_all_tr_state_lifecycle(
            &default_tr_state_lifecycle_checks(),
            &outcome.records,
            &prior_tsr,
            &ctx,
        );
        info!(lfc_issues = lfc_issues.len(), "TSR lifecycle checks run");
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

fn run_mar_scan(input: &Path, store_path: Option<&Path>, out: &Path) -> Result<()> {
    let started_at = Utc::now();
    let outcome = read_emir_mar_xml(input)
        .with_context(|| format!("reading MAR file {}", input.display()))?;
    info!(
        file = %input.display(),
        records = outcome.records.len(),
        format_issues = outcome.issues.len(),
        "loaded EMIR MAR XML",
    );

    let mut issues: Vec<DqIssue> = outcome.issues;

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
    let mar_issues = run_all_margin_activity(
        &default_margin_activity_checks(),
        &outcome.records,
        &prior,
        &ctx,
    );
    info!(mar_issues = mar_issues.len(), "MAR checks run");
    issues.extend(mar_issues);
    if !prior.is_empty() {
        let lfc_issues = run_all_margin_activity(
            &default_margin_activity_lifecycle_checks(),
            &outcome.records,
            &prior,
            &ctx,
        );
        info!(lfc_issues = lfc_issues.len(), "MAR lifecycle checks run");
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
    write_issues_csv(&out.join("mar_issues.csv"), &issues)?;
    write_report_html(&out.join("mar_report.html"), &summary, &issues, &sources)?;

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

fn run_msr_scan(input: &Path, store_path: Option<&Path>, out: &Path) -> Result<()> {
    let started_at = Utc::now();
    let outcome = read_emir_msr_xml(input)
        .with_context(|| format!("reading MSR file {}", input.display()))?;
    info!(
        file = %input.display(),
        records = outcome.records.len(),
        format_issues = outcome.issues.len(),
        "loaded EMIR MSR XML",
    );

    let mut issues: Vec<DqIssue> = outcome.issues;

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
    let msr_issues = run_all_margin_state(
        &default_margin_state_checks(),
        &outcome.records,
        &prior,
        &ctx,
    );
    info!(msr_issues = msr_issues.len(), "MSR checks run");
    issues.extend(msr_issues);
    if !prior_msr.is_empty() {
        let lfc_issues = run_all_margin_state_lifecycle(
            &default_margin_state_lifecycle_checks(),
            &outcome.records,
            &prior_msr,
            &ctx,
        );
        info!(lfc_issues = lfc_issues.len(), "MSR lifecycle checks run");
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
    write_issues_csv(&out.join("msr_issues.csv"), &issues)?;
    write_report_html(&out.join("msr_report.html"), &summary, &issues, &sources)?;

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

    let activity_issues = run_all_tr_activity(
        &default_tr_activity_checks(),
        &records,
        &prior,
        tsr_records.as_deref(),
        &ctx,
    );
    info!(activity_issues = activity_issues.len(), "TAR checks run");

    let mut issues: Vec<DqIssue> = format_issues;
    issues.extend(activity_issues);
    finalize_issues(&mut issues, &ctx);

    let summary = build_summary(&records, &issues, &inputs, started_at, Utc::now());

    std::fs::create_dir_all(out)
        .with_context(|| format!("creating output directory {}", out.display()))?;
    write_summary_json(&out.join("summary.json"), &summary)?;
    // Activity-specific summary embedding distributions (sibling
    // file so the standard `summary.json` remains regime-uniform).
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

fn run_tr_audit(
    tar_path: &Path,
    tsr_path: &Path,
    feedback_path: &Path,
    store_path: Option<&Path>,
    out: &Path,
) -> Result<()> {
    use std::collections::HashSet;

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

    let mut issues: Vec<DqIssue> = tar_issues;
    issues.extend(tsr_outcome.issues.clone());
    issues.extend(feedback_outcome.issues.clone());

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

    // 3. Run each layer's checks.
    let tar_checks = run_all(&default_checks(), &tar_records, &ctx);
    issues.extend(tar_checks);
    let lifecycle = run_all_lifecycle(&default_lifecycle_checks(), &tar_records, &prior, &ctx);
    issues.extend(lifecycle);
    let tsr_checks = run_all_tr_state(
        &default_tr_state_checks(),
        &tsr_outcome.records,
        &prior,
        &ctx,
    );
    issues.extend(tsr_checks);
    let fbk_checks = run_all_feedback(
        &default_feedback_checks(),
        &feedback_outcome.records,
        &prior,
        &ctx,
    );
    issues.extend(fbk_checks);
    let activity_checks = run_all_tr_activity(
        &default_tr_activity_checks(),
        &tar_records,
        &prior,
        Some(&tsr_outcome.records),
        &ctx,
    );
    issues.extend(activity_checks);

    // 4. Cross-layer coherence checks (EMIR.AUD.*).
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

    // EMIR.AUD.NEWT_IN_TAR_NOT_IN_TSR
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
                    check_id: "EMIR.AUD.NEWT_IN_TAR_NOT_IN_TSR".into(),
                    regime: Regime::Emir,
                    severity: Severity::High,
                    dimension: DqDimension::Consistency,
                    record_id: r.record_id.clone(),
                    uti: Some(uti.to_owned()),
                    field: Some("uti".into()),
                    value: Some(uti.to_owned()),
                    message: format!(
                        "UTI {uti} was NEWT'd in the TAR but is absent from the TSR — submission may not have been accepted."
                    ),
                    source_file: r.source_file.clone(),
                });
            }
        }
    }

    // EMIR.AUD.OUTSTANDING_IN_TSR_NOT_IN_TAR
    for uti in tsr_outstanding_utis.iter() {
        if !tar_utis.contains(uti) {
            issues.push(DqIssue {
                check_id: "EMIR.AUD.OUTSTANDING_IN_TSR_NOT_IN_TAR".into(),
                regime: Regime::Emir,
                severity: Severity::Warning,
                dimension: DqDimension::Consistency,
                record_id: None,
                uti: Some((*uti).to_owned()),
                field: Some("uti".into()),
                value: Some((*uti).to_owned()),
                message: format!(
                    "UTI {uti} is outstanding in the TSR but no record appears in the TAR for this period."
                ),
                source_file: Some(tsr_path.to_string_lossy().into_owned()),
            });
        }
    }

    // EMIR.AUD.REJECTED_BUT_OUTSTANDING_IN_TSR
    for uti in rejected_utis.iter() {
        if tsr_outstanding_utis.contains(uti) {
            issues.push(DqIssue {
                check_id: "EMIR.AUD.REJECTED_BUT_OUTSTANDING_IN_TSR".into(),
                regime: Regime::Emir,
                severity: Severity::Critical,
                dimension: DqDimension::Consistency,
                record_id: None,
                uti: Some((*uti).to_owned()),
                field: Some("uti".into()),
                value: Some((*uti).to_owned()),
                message: format!(
                    "UTI {uti} is reported rejected in the feedback file yet appears outstanding in the TSR — TR-side inconsistency."
                ),
                source_file: Some(feedback_path.to_string_lossy().into_owned()),
            });
        }
    }

    finalize_issues(&mut issues, &ctx);

    // 5. Write outputs.
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

/// Valuation-mismatch threshold (relative): book vs TSR must agree
/// within 1% on `valuation_amount`. Compile-time constant in v1;
/// will migrate to `Thresholds` config when a real profile emerges.
const BOOK_VALUATION_TOLERANCE_PCT: f64 = 1.0;

fn run_book_reconcile(
    book_path: &Path,
    tsr_path: &Path,
    mapping_path: &Path,
    out: &Path,
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

    let mut issues: Vec<DqIssue> = tsr_outcome.issues.clone();
    issues.extend(compute_book_reconcile_issues(
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

/// Pure function that compares a book batch (as `EmirRecord`s) to a
/// TSR snapshot (`TrStateRecord`s) and produces the 7
/// `EMIR.BREC.*` issues. Extracted from the CLI runner for testability.
fn compute_book_reconcile_issues(
    book: &[EmirRecord],
    tsr: &[opendqi_core::TrStateRecord],
) -> Vec<DqIssue> {
    use std::collections::BTreeMap;

    fn uti_key(s: &Option<String>) -> Option<&str> {
        s.as_deref().map(str::trim).filter(|t| !t.is_empty())
    }

    let book_by_uti: BTreeMap<&str, &EmirRecord> = book
        .iter()
        .filter_map(|r| uti_key(&r.uti).map(|u| (u, r)))
        .collect();
    let tsr_by_uti: BTreeMap<&str, &opendqi_core::TrStateRecord> = tsr
        .iter()
        .filter_map(|r| uti_key(&r.uti).map(|u| (u, r)))
        .collect();

    let mut all_utis: Vec<&str> = book_by_uti
        .keys()
        .chain(tsr_by_uti.keys())
        .copied()
        .collect();
    all_utis.sort();
    all_utis.dedup();

    let mut out = Vec::new();
    for uti in all_utis {
        match (book_by_uti.get(uti), tsr_by_uti.get(uti)) {
            (Some(b), None) => {
                out.push(brec_issue(
                    "EMIR.BREC.IN_BOOK_NOT_IN_TSR",
                    Severity::High,
                    DqDimension::Consistency,
                    uti,
                    b.record_id.clone(),
                    b.source_file.clone(),
                    Some("uti".into()),
                    None,
                    format!(
                        "UTI {uti} appears in the firm's book but is absent from the TR State Report."
                    ),
                ));
            }
            (None, Some(t)) => {
                if !tsr_is_outstanding(t) {
                    continue;
                }
                out.push(brec_issue(
                    "EMIR.BREC.IN_TSR_NOT_IN_BOOK",
                    Severity::High,
                    DqDimension::Consistency,
                    uti,
                    t.record_id.clone(),
                    t.source_file.clone(),
                    Some("uti".into()),
                    None,
                    format!(
                        "UTI {uti} is outstanding at the TR but is absent from the firm's book."
                    ),
                ));
            }
            (Some(b), Some(t)) => {
                // Notional amount mismatch.
                if let (Some(bn), Some(tn)) = (b.notional_amount, t.notional_amount) {
                    if bn != tn {
                        out.push(brec_issue(
                            "EMIR.BREC.NOTIONAL_MISMATCH",
                            Severity::High,
                            DqDimension::Accuracy,
                            uti,
                            b.record_id.clone(),
                            b.source_file.clone(),
                            Some("notional_amount".into()),
                            Some(format!("book={bn} tsr={tn}")),
                            format!("Notional mismatch on UTI {uti}: book {bn} vs TSR {tn}."),
                        ));
                    }
                }
                // Notional currency mismatch.
                if let (Some(bc), Some(tc)) = (
                    b.notional_currency.as_deref(),
                    t.notional_currency.as_deref(),
                ) {
                    if !bc.eq_ignore_ascii_case(tc) {
                        out.push(brec_issue(
                            "EMIR.BREC.NOTIONAL_CURRENCY_MISMATCH",
                            Severity::Warning,
                            DqDimension::Validity,
                            uti,
                            b.record_id.clone(),
                            b.source_file.clone(),
                            Some("notional_currency".into()),
                            Some(format!("book={bc} tsr={tc}")),
                            format!(
                                "Notional currency mismatch on UTI {uti}: book {bc} vs TSR {tc}."
                            ),
                        ));
                    }
                }
                // Valuation mismatch (relative threshold).
                if let (Some(bv), Some(tv)) = (b.valuation_amount, t.valuation_amount) {
                    if !valuation_within_tolerance(bv, tv) {
                        out.push(brec_issue(
                            "EMIR.BREC.VALUATION_MISMATCH",
                            Severity::Warning,
                            DqDimension::Accuracy,
                            uti,
                            b.record_id.clone(),
                            b.source_file.clone(),
                            Some("valuation_amount".into()),
                            Some(format!("book={bv} tsr={tv}")),
                            format!(
                                "Valuation mismatch on UTI {uti}: book {bv} vs TSR {tv} (tolerance {BOOK_VALUATION_TOLERANCE_PCT}%)."
                            ),
                        ));
                    }
                }
                // Maturity mismatch.
                if let (Some(bm), Some(tm)) = (b.maturity_date, t.maturity_date) {
                    if bm != tm {
                        out.push(brec_issue(
                            "EMIR.BREC.MATURITY_MISMATCH",
                            Severity::High,
                            DqDimension::Accuracy,
                            uti,
                            b.record_id.clone(),
                            b.source_file.clone(),
                            Some("maturity_date".into()),
                            Some(format!("book={bm} tsr={tm}")),
                            format!("Maturity mismatch on UTI {uti}: book {bm} vs TSR {tm}."),
                        ));
                    }
                }
                // Status mismatch: book sees the trade as active (no
                // termination_date) but the TSR reports it as
                // terminated. (We don't fire the inverse: book
                // terminated vs TSR active rarely warrants a warning.)
                let book_active = b.termination_date.is_none();
                let tsr_terminated = t
                    .status
                    .as_deref()
                    .map(|s| s.trim().eq_ignore_ascii_case("TERMINATED"))
                    .unwrap_or(false);
                if book_active && tsr_terminated {
                    out.push(brec_issue(
                        "EMIR.BREC.STATUS_MISMATCH",
                        Severity::Warning,
                        DqDimension::Consistency,
                        uti,
                        b.record_id.clone(),
                        b.source_file.clone(),
                        Some("status".into()),
                        Some("book=active tsr=TERMINATED".into()),
                        format!(
                            "Status mismatch on UTI {uti}: book has no termination but TSR reports the trade as TERMINATED."
                        ),
                    ));
                }
            }
            (None, None) => unreachable!(),
        }
    }
    out
}

fn tsr_is_outstanding(t: &opendqi_core::TrStateRecord) -> bool {
    match t.status.as_deref() {
        None => true,
        Some(s) => {
            let s = s.trim();
            s.is_empty()
                || s.eq_ignore_ascii_case("OUTSTANDING")
                || s.eq_ignore_ascii_case("ACTIVE")
                || s.eq_ignore_ascii_case("LIVE")
        }
    }
}

fn valuation_within_tolerance(book: rust_decimal::Decimal, tsr: rust_decimal::Decimal) -> bool {
    if book == tsr {
        return true;
    }
    let book_f = book.to_string().parse::<f64>().unwrap_or(f64::NAN);
    let tsr_f = tsr.to_string().parse::<f64>().unwrap_or(f64::NAN);
    if !book_f.is_finite() || !tsr_f.is_finite() {
        return false;
    }
    if tsr_f.abs() < f64::EPSILON {
        return book_f.abs() < f64::EPSILON;
    }
    let diff_pct = ((book_f - tsr_f).abs() / tsr_f.abs()) * 100.0;
    diff_pct <= BOOK_VALUATION_TOLERANCE_PCT
}

#[allow(clippy::too_many_arguments)]
fn brec_issue(
    check_id: &str,
    severity: Severity,
    dimension: DqDimension,
    uti: &str,
    record_id: Option<String>,
    source_file: Option<String>,
    field: Option<String>,
    value: Option<String>,
    message: String,
) -> DqIssue {
    DqIssue {
        check_id: check_id.into(),
        regime: Regime::Emir,
        severity,
        dimension,
        record_id,
        uti: Some(uti.to_owned()),
        field,
        value,
        message,
        source_file,
    }
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

fn build_summary(
    records: &[EmirRecord],
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
        regime: Regime::Emir,
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
