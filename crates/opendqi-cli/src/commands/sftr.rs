//! `opendqi sftr ...` subcommands.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use clap::Subcommand;
use opendqi_core::dq::{
    default_sftr_checks, default_sftr_lifecycle_checks, run_all_sftr, run_all_sftr_lifecycle,
    CheckContext,
};
use opendqi_core::{DqDimension, DqIssue, Regime, ScanSummary, Severity, SftrRecord, Thresholds};
use opendqi_io::{discover_emir_inputs, has_extension};
use opendqi_report::{write_issues_csv, write_report_html, write_summary_json};
use opendqi_xml::{
    check_wellformedness, read_sftr_xml, ExternalXmllintValidator, XsdValidator, XsdViolation,
};
use tracing::{info, warn};

const CHECK_XSD_VIOLATION: &str = "SFTR.FMT.XSD_VIOLATION";
const CHECK_XSD_TOOL_ERROR: &str = "SFTR.FMT.XSD_TOOL_ERROR";

#[derive(Subcommand)]
pub enum SftrAction {
    /// Run the full DQ scan over an SFTR XML input (file or directory).
    Scan {
        /// Path to an XML file or a directory containing XML files.
        input: PathBuf,
        /// Directory where reports are written. Created if absent.
        #[arg(long)]
        out: PathBuf,
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
    /// Normalize an SFTR file into Parquet — planned milestone.
    Normalize {
        /// Path to the input file.
        input: PathBuf,
        /// Parquet output path.
        #[arg(long)]
        out: PathBuf,
    },
}

pub fn run(action: SftrAction) -> Result<ExitCode> {
    match action {
        SftrAction::Scan {
            input,
            out,
            config,
            xsd,
            store,
        } => {
            run_scan(
                &input,
                &out,
                config.as_deref(),
                xsd.as_deref(),
                store.as_deref(),
            )?;
            Ok(ExitCode::SUCCESS)
        }
        SftrAction::Validate { input, xsd } => run_validate(&input, &xsd),
        SftrAction::Normalize { .. } => {
            println!(
                "opendqi sftr normalize: Parquet normalization is planned for a future milestone."
            );
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn run_scan(
    input: &Path,
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
        return Err(anyhow!("no XML inputs found at {}", input.display()));
    }
    info!(count = inputs.len(), "discovered inputs");

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
        } else {
            warn!(path = %path.display(), "unsupported file extension; only XML is supported by opendqi sftr scan");
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

fn sort_issues(issues: &mut [DqIssue]) {
    issues.sort_by(|a, b| {
        a.check_id
            .cmp(&b.check_id)
            .then_with(|| a.source_file.cmp(&b.source_file))
            .then_with(|| a.record_id.cmp(&b.record_id))
    });
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
