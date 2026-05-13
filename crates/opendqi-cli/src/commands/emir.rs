//! `opendqi emir ...` subcommands.

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use clap::Subcommand;
use opendqi_core::dq::{default_checks, run_all, CheckContext};
use opendqi_core::{DqDimension, DqIssue, EmirRecord, Regime, ScanSummary, Severity, Thresholds};
use opendqi_io::{discover_inputs, read_emir_csv, CsvMapping};
use opendqi_report::{write_issues_csv, write_report_html, write_summary_json};
use tracing::info;

#[derive(Subcommand)]
pub enum EmirAction {
    /// Run the full DQ scan over a CSV (file or directory of CSVs).
    Scan {
        /// Path to a CSV file or a directory containing CSV files.
        input: PathBuf,
        /// Path to the YAML mapping describing the CSV columns.
        #[arg(long)]
        mapping: PathBuf,
        /// Directory where reports are written. Created if absent.
        #[arg(long)]
        out: PathBuf,
        /// Optional YAML thresholds configuration.
        #[arg(long)]
        config: Option<PathBuf>,
    },
    /// Validate file structure (XML well-formedness, XSD) — planned for 0.3.
    Validate {
        /// Path to the input file.
        input: PathBuf,
        /// Path to an XSD schema directory.
        #[arg(long)]
        xsd: Option<PathBuf>,
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

pub fn run(action: EmirAction) -> Result<()> {
    match action {
        EmirAction::Scan {
            input,
            mapping,
            out,
            config,
        } => run_scan(&input, &mapping, &out, config.as_deref()),
        EmirAction::Validate { .. } => {
            println!("opendqi emir validate: XML/XSD validation is planned for milestone 0.3.");
            Ok(())
        }
        EmirAction::Normalize { .. } => {
            println!("opendqi emir normalize: Parquet normalization is planned for milestone 0.2.");
            Ok(())
        }
    }
}

fn run_scan(
    input: &std::path::Path,
    mapping_path: &std::path::Path,
    out: &std::path::Path,
    config_path: Option<&std::path::Path>,
) -> Result<()> {
    let started_at = Utc::now();

    let mapping = CsvMapping::from_path(mapping_path)?;
    let thresholds = match config_path {
        Some(p) => {
            let text = std::fs::read_to_string(p)
                .with_context(|| format!("reading thresholds config {}", p.display()))?;
            serde_yaml::from_str::<Thresholds>(&text)
                .with_context(|| format!("parsing thresholds config {}", p.display()))?
        }
        None => Thresholds::default(),
    };

    let inputs = discover_inputs(input)?;
    if inputs.is_empty() {
        return Err(anyhow!("no CSV inputs found at {}", input.display()));
    }
    info!(count = inputs.len(), "discovered inputs");

    let mut records: Vec<EmirRecord> = Vec::new();
    let mut sources: Vec<String> = Vec::with_capacity(inputs.len());
    for path in &inputs {
        let mut batch = read_emir_csv(path, &mapping)?;
        info!(file = %path.display(), rows = batch.len(), "loaded CSV");
        sources.push(path.to_string_lossy().into_owned());
        records.append(&mut batch);
    }

    let now = Utc::now();
    let ctx = CheckContext {
        thresholds,
        today: now.date_naive(),
        now,
    };

    let checks = default_checks();
    let issues = run_all(&checks, &records, &ctx);

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
