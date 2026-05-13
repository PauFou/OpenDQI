//! `opendqi emir ...` subcommands.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use clap::Subcommand;
use opendqi_core::dq::{
    default_checks, default_feedback_checks, default_lifecycle_checks,
    default_reconciliation_checks, default_tr_state_checks, run_all, run_all_feedback,
    run_all_lifecycle, run_all_reconciliation, run_all_tr_state, CheckContext,
};
use opendqi_core::{DqDimension, DqIssue, EmirRecord, Regime, ScanSummary, Severity, Thresholds};
use opendqi_io::{discover_emir_inputs, has_extension, read_emir_csv, CsvMapping};
use opendqi_report::{write_issues_csv, write_report_html, write_summary_json};
use opendqi_xml::{
    check_wellformedness, read_emir_feedback_xml, read_emir_reconciliation_xml,
    read_emir_tr_state_xml, read_emir_xml, ExternalXmllintValidator, XsdValidator, XsdViolation,
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
    /// Normalize input into a canonical Parquet file — planned for 0.2.
    Normalize {
        /// Path to the input file.
        input: PathBuf,
        /// Output Parquet path.
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
        EmirAction::Normalize { .. } => {
            println!("opendqi emir normalize: Parquet normalization is planned for milestone 0.2.");
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

    sort_issues(&mut issues);

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
    sort_issues(&mut issues);

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
    sort_issues(&mut issues);

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

    // Optional store enrichment: load prior EMIR records for the
    // UTIs present in the TSR (not used by v1 checks but the API
    // is symmetric and forward-compatible).
    let prior: Vec<EmirRecord> = if let Some(store_path) = store_path {
        let store = opendqi_store::open_store(store_path)
            .with_context(|| format!("opening history store at {}", store_path.display()))?;
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
    let tsr_issues = run_all_tr_state(&default_tr_state_checks(), &outcome.records, &prior, &ctx);
    info!(tsr_issues = tsr_issues.len(), "TSR checks run");
    issues.extend(tsr_issues);
    sort_issues(&mut issues);

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

fn sort_issues(issues: &mut [DqIssue]) {
    issues.sort_by(|a, b| {
        a.check_id
            .cmp(&b.check_id)
            .then_with(|| a.source_file.cmp(&b.source_file))
            .then_with(|| a.record_id.cmp(&b.record_id))
    });
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
