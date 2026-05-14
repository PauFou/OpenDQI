//! Server-side scan orchestration. Mirrors a stripped-down version of
//! `opendqi-cli::commands::{emir,sftr}::run_scan` — XML and Parquet
//! only (no CSV mapping upload v1). Writes the standard report trio
//! (`summary.json`, `issues.csv`, `report.html`) into the provided
//! output directory.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use opendqi_core::dq::{default_checks, default_sftr_checks, run_all, run_all_sftr, CheckContext};
use opendqi_core::{
    DqDimension, DqIssue, EmirRecord, Regime, ScanSummary, Severity, SftrRecord, Thresholds,
};
use opendqi_io::{has_extension, read_emir_parquet, read_sftr_parquet};
use opendqi_report::{write_issues_csv, write_report_html, write_summary_json};
use opendqi_xml::{read_emir_xml, read_sftr_xml};

/// What the user picked in the form.
#[derive(Debug, Clone, Copy)]
pub enum UiRegime {
    Emir,
    Sftr,
}

impl UiRegime {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "emir" => Some(Self::Emir),
            "sftr" => Some(Self::Sftr),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Emir => "EMIR",
            Self::Sftr => "SFTR",
        }
    }
}

/// Result of one server-side scan, ready for the results template.
#[derive(Debug)]
pub struct ScanArtifacts {
    pub regime: UiRegime,
    pub records: u32,
    pub issues_total: u32,
    pub issues_critical: u32,
    pub issues_high: u32,
    pub quality_score: f32,
    pub artifact_files: Vec<String>,
}

/// Run a scan over the uploaded `input` file and write the report
/// artifacts into `out_dir`. The `out_dir` must already exist.
pub fn run_server_scan(input: &Path, regime: UiRegime, out_dir: &Path) -> Result<ScanArtifacts> {
    let started_at = Utc::now();
    let sources = vec![input.to_string_lossy().into_owned()];

    let (records_processed, summary, issues) = match regime {
        UiRegime::Emir => {
            let records = load_emir(input)?;
            let now = Utc::now();
            let ctx = CheckContext {
                thresholds: Thresholds::default(),
                today: now.date_naive(),
                now,
            };
            let mut issues = run_all(&default_checks(), &records, &ctx);
            sort_issues(&mut issues);
            let summary = build_summary(
                Regime::Emir,
                records.len() as u32,
                &issues,
                1,
                started_at,
                Utc::now(),
            );
            (records.len() as u32, summary, issues)
        }
        UiRegime::Sftr => {
            let records = load_sftr(input)?;
            let now = Utc::now();
            let ctx = CheckContext {
                thresholds: Thresholds::default(),
                today: now.date_naive(),
                now,
            };
            let mut issues = run_all_sftr(&default_sftr_checks(), &records, &ctx);
            sort_issues(&mut issues);
            let summary = build_summary(
                Regime::Sftr,
                records.len() as u32,
                &issues,
                1,
                started_at,
                Utc::now(),
            );
            (records.len() as u32, summary, issues)
        }
    };

    write_summary_json(&out_dir.join("summary.json"), &summary).context("writing summary.json")?;
    write_issues_csv(&out_dir.join("issues.csv"), &issues).context("writing issues.csv")?;
    write_report_html(&out_dir.join("report.html"), &summary, &issues, &sources)
        .context("writing report.html")?;

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

    let artifact_files = vec![
        "report.html".to_string(),
        "summary.json".to_string(),
        "issues.csv".to_string(),
    ];

    Ok(ScanArtifacts {
        regime,
        records: records_processed,
        issues_total: summary.issues_total,
        issues_critical: critical,
        issues_high: high,
        quality_score: summary.quality_score,
        artifact_files,
    })
}

fn load_emir(input: &Path) -> Result<Vec<EmirRecord>> {
    if has_extension(input, "xml") {
        let outcome = read_emir_xml(input)
            .with_context(|| format!("reading EMIR XML {}", input.display()))?;
        Ok(outcome.records)
    } else if has_extension(input, "parquet") {
        read_emir_parquet(input)
            .with_context(|| format!("reading EMIR Parquet {}", input.display()))
    } else {
        Err(anyhow!(
            "unsupported input extension for EMIR (allowed: .xml, .parquet): {}",
            input.display()
        ))
    }
}

fn load_sftr(input: &Path) -> Result<Vec<SftrRecord>> {
    if has_extension(input, "xml") {
        let outcome = read_sftr_xml(input)
            .with_context(|| format!("reading SFTR XML {}", input.display()))?;
        Ok(outcome.records)
    } else if has_extension(input, "parquet") {
        read_sftr_parquet(input)
            .with_context(|| format!("reading SFTR Parquet {}", input.display()))
    } else {
        Err(anyhow!(
            "unsupported input extension for SFTR (allowed: .xml, .parquet): {}",
            input.display()
        ))
    }
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
    regime: Regime,
    records: u32,
    issues: &[DqIssue],
    files: u32,
    started_at: chrono::DateTime<Utc>,
    finished_at: chrono::DateTime<Utc>,
) -> ScanSummary {
    use std::collections::BTreeMap;
    let mut by_sev: BTreeMap<Severity, u32> = BTreeMap::new();
    let mut by_dim: BTreeMap<DqDimension, u32> = BTreeMap::new();
    for i in issues {
        *by_sev.entry(i.severity).or_insert(0) += 1;
        *by_dim.entry(i.dimension).or_insert(0) += 1;
    }
    ScanSummary {
        regime,
        files_processed: files,
        records_processed: records,
        issues_total: issues.len() as u32,
        issues_by_severity: by_sev,
        issues_by_dimension: by_dim,
        quality_score: opendqi_core::quality_score(records, issues),
        started_at,
        finished_at,
    }
}

/// Resolve the original filename to a sane safe stem suitable for
/// writing into the per-scan directory. Strips any path component the
/// browser may have included.
pub fn sanitize_upload_filename(name: &str) -> String {
    let stem = std::path::Path::new(name)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "upload.bin".to_string());
    if stem.is_empty() {
        "upload.bin".into()
    } else {
        stem
    }
}

/// Create a scan-id-scoped output directory under `base`, returning
/// `(scan_id, path)`. The id is a v4 UUID.
pub fn new_scan_dir(base: &Path) -> Result<(String, PathBuf)> {
    let id = uuid::Uuid::new_v4().to_string();
    let dir = base.join(&id);
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    Ok((id, dir))
}
