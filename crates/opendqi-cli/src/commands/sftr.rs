//! `opendqi sftr ...` subcommands.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use clap::Subcommand;
use opendqi_core::dq::{default_sftr_checks, run_all_sftr, CheckContext};
use opendqi_core::{DqDimension, DqIssue, Regime, ScanSummary, Severity, SftrRecord, Thresholds};
use opendqi_io::{discover_emir_inputs, has_extension};
use opendqi_report::{write_issues_csv, write_report_html, write_summary_json};
use opendqi_xml::read_sftr_xml;
use tracing::{info, warn};

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
    },
    /// Validate an SFTR file against its schema — planned milestone.
    Validate {
        /// Path to the input file.
        input: PathBuf,
        /// XSD schema directory.
        #[arg(long)]
        xsd: Option<PathBuf>,
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

pub fn run(action: SftrAction) -> Result<()> {
    match action {
        SftrAction::Scan { input, out, config } => run_scan(&input, &out, config.as_deref()),
        SftrAction::Validate { .. } => {
            println!(
                "opendqi sftr validate: XML/XSD validation is planned for a future milestone."
            );
            Ok(())
        }
        SftrAction::Normalize { .. } => {
            println!(
                "opendqi sftr normalize: Parquet normalization is planned for a future milestone."
            );
            Ok(())
        }
    }
}

fn run_scan(input: &Path, out: &Path, config_path: Option<&Path>) -> Result<()> {
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
                format_issues = outcome.issues.len(),
                "loaded SFTR XML",
            );
            records.append(&mut outcome.records);
            format_issues.append(&mut outcome.issues);
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
    sort_issues(&mut issues);

    let summary = build_summary(&records, &issues, &inputs, started_at, Utc::now());

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
