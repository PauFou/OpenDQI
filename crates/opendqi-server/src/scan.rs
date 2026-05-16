//! Server-side scan orchestration. Mirrors a stripped-down version of
//! `opendqi-cli::commands::{emir,sftr}::run_scan` — XML and Parquet
//! only (no CSV mapping upload v1). Writes the standard report trio
//! (`summary.json`, `issues.csv`, `report.html`) into the provided
//! output directory.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use opendqi_core::dq::{
    default_checks, default_feedback_checks, default_margin_activity_checks,
    default_margin_state_checks, default_recon_stats_checks, default_reconciliation_checks,
    default_sftr_checks, default_sftr_tr_activity_checks, default_sftr_tr_state_checks,
    default_tr_activity_checks, default_tr_state_checks, finalize_issues, run_all,
    run_all_feedback, run_all_margin_activity, run_all_margin_state, run_all_recon_stats,
    run_all_reconciliation, run_all_sftr, run_all_sftr_tr_activity, run_all_sftr_tr_state,
    run_all_tr_activity, run_all_tr_state, CheckContext,
};
use opendqi_core::{
    DqDimension, DqIssue, EmirRecord, Regime, ScanSummary, Severity, SftrRecord, Thresholds,
};
use opendqi_io::{has_extension, read_emir_parquet, read_sftr_parquet};
use opendqi_report::{write_issues_csv, write_report_html, write_summary_json};
use opendqi_xml::{
    check_wellformedness, read_emir_feedback_xml, read_emir_mar_xml, read_emir_msr_xml,
    read_emir_recon_stats_xml, read_emir_tr_state_xml, read_emir_xml, read_sftr_tr_state_xml,
    read_sftr_xml,
};

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

/// Which scan operation the user picked. `Scan` runs the standard
/// submission DQ check pack (default for backward compat). The other
/// variants route to TR-layer scan helpers.
#[derive(Debug, Clone, Copy)]
pub enum UiOperation {
    /// Standard submission scan (`opendqi {emir,sftr} scan`).
    Scan,
    /// Trade State Report scan (auth.107 EMIR / auth.079 SFTR).
    TrStateScan,
    /// Trade Activity Report scan (auth.030 EMIR / auth.052 SFTR
    /// replay).
    TrActivityScan,
    /// Feedback file scan (auth.092 EMIR / auth.080 SFTR). v1
    /// runs the well-formedness / namespace / parse checks only —
    /// store-cross-referenced FBK checks remain CLI-only.
    Feedback,
    /// EMIR Reconciliation Statistics scan (auth.091). EMIR-only.
    ReconStats,
    /// EMIR Margin Activity Report scan (auth.108). EMIR-only.
    MarScan,
    /// EMIR Margin State Report scan (auth.109). EMIR-only.
    MsrScan,
    /// XML well-formedness validation (regime-agnostic). v1 doesn't
    /// expose an XSD picker; the schema validation remains CLI-only.
    Validate,
    /// Book-vs-TSR reconciliation. Multi-file: `file_book` (CSV),
    /// `file_tsr` (auth.107/auth.079 XML), `file_mapping` (YAML).
    BookReconcile,
    /// Consolidated TR audit. Multi-file: `file_tar` (auth.030/052
    /// XML), `file_tsr` (auth.107/079 XML), `file_feedback`
    /// (auth.092/080 XML). No store (web UI v1): lifecycle checks
    /// remain CLI-only.
    TrAudit,
}

impl UiOperation {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "scan" => Some(Self::Scan),
            "tr-state-scan" | "tr_state_scan" => Some(Self::TrStateScan),
            "tr-activity-scan" | "tr_activity_scan" => Some(Self::TrActivityScan),
            "feedback" => Some(Self::Feedback),
            "recon-stats" | "recon_stats" => Some(Self::ReconStats),
            "mar-scan" | "mar_scan" => Some(Self::MarScan),
            "msr-scan" | "msr_scan" => Some(Self::MsrScan),
            "validate" => Some(Self::Validate),
            "book-reconcile" | "book_reconcile" => Some(Self::BookReconcile),
            "tr-audit" | "tr_audit" => Some(Self::TrAudit),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Scan => "scan",
            Self::TrStateScan => "tr-state-scan",
            Self::TrActivityScan => "tr-activity-scan",
            Self::Feedback => "feedback",
            Self::ReconStats => "recon-stats",
            Self::MarScan => "mar-scan",
            Self::MsrScan => "msr-scan",
            Self::Validate => "validate",
            Self::BookReconcile => "book-reconcile",
            Self::TrAudit => "tr-audit",
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

/// Dispatch the chosen `operation` to the appropriate scan helper.
/// Output artifacts always land under names the results page knows:
/// `summary.json` / `issues.csv` / `report.html`.
pub fn run_server_operation(
    input: &Path,
    regime: UiRegime,
    operation: UiOperation,
    out_dir: &Path,
) -> Result<ScanArtifacts> {
    match operation {
        UiOperation::Scan => run_server_scan(input, regime, out_dir),
        UiOperation::TrStateScan => match regime {
            UiRegime::Emir => run_emir_tr_state_server(input, out_dir),
            UiRegime::Sftr => run_sftr_tr_state_server(input, out_dir),
        },
        UiOperation::TrActivityScan => match regime {
            UiRegime::Emir => run_emir_tr_activity_server(input, out_dir),
            UiRegime::Sftr => run_sftr_tr_activity_server(input, out_dir),
        },
        UiOperation::Feedback => match regime {
            UiRegime::Emir => run_emir_feedback_server(input, out_dir),
            UiRegime::Sftr => Err(anyhow!(
                "SFTR has no rejection-feedback message — real auth.080 is a \
                 reconciliation status advice; use the SFTR reconcile operation."
            )),
        },
        UiOperation::ReconStats => match regime {
            UiRegime::Emir => run_emir_recon_stats_server(input, out_dir),
            UiRegime::Sftr => Err(anyhow!(
                "recon-stats is EMIR-only (auth.091); pick a different operation for SFTR."
            )),
        },
        UiOperation::MarScan => match regime {
            UiRegime::Emir => run_emir_mar_server(input, out_dir),
            UiRegime::Sftr => Err(anyhow!(
                "mar-scan is EMIR-only (auth.108); pick a different operation for SFTR."
            )),
        },
        UiOperation::MsrScan => match regime {
            UiRegime::Emir => run_emir_msr_server(input, out_dir),
            UiRegime::Sftr => Err(anyhow!(
                "msr-scan is EMIR-only (auth.109); pick a different operation for SFTR."
            )),
        },
        UiOperation::Validate => run_validate_server(input, regime, out_dir),
        UiOperation::BookReconcile | UiOperation::TrAudit => Err(anyhow!(
            "{} is multi-file; route via run_server_dispatch",
            operation.as_str()
        )),
    }
}

/// Multi-file aware entry point. `saved` maps each multipart field
/// name to the file path it was written to in `out_dir`. Single-file
/// operations consume the `file` entry; `BookReconcile` consumes
/// `file_book` / `file_tsr` / `file_mapping`.
pub fn run_server_dispatch(
    saved: &std::collections::BTreeMap<String, PathBuf>,
    regime: UiRegime,
    operation: UiOperation,
    out_dir: &Path,
) -> Result<ScanArtifacts> {
    match operation {
        UiOperation::BookReconcile => {
            let book = saved
                .get("file_book")
                .ok_or_else(|| anyhow!("book-reconcile requires a `file_book` (CSV) upload"))?;
            let tsr = saved
                .get("file_tsr")
                .ok_or_else(|| anyhow!("book-reconcile requires a `file_tsr` (XML) upload"))?;
            let mapping = saved
                .get("file_mapping")
                .ok_or_else(|| anyhow!("book-reconcile requires a `file_mapping` (YAML) upload"))?;
            match regime {
                UiRegime::Emir => run_emir_book_reconcile_server(book, tsr, mapping, out_dir),
                UiRegime::Sftr => run_sftr_book_reconcile_server(book, tsr, mapping, out_dir),
            }
        }
        UiOperation::TrAudit => {
            let tar = saved
                .get("file_tar")
                .ok_or_else(|| anyhow!("tr-audit requires a `file_tar` (XML) upload"))?;
            let tsr = saved
                .get("file_tsr")
                .ok_or_else(|| anyhow!("tr-audit requires a `file_tsr` (XML) upload"))?;
            match regime {
                UiRegime::Emir => {
                    let feedback = saved.get("file_feedback").ok_or_else(|| {
                        anyhow!("EMIR tr-audit requires a `file_feedback` (XML) upload")
                    })?;
                    run_emir_tr_audit_server(tar, tsr, feedback, out_dir)
                }
                // SFTR has no rejection-feedback message → TAR+TSR only.
                UiRegime::Sftr => run_sftr_tr_audit_server(tar, tsr, out_dir),
            }
        }
        _ => {
            // Single-file operations: take the `file` part (fall back
            // to the sole uploaded file if the field was named
            // differently).
            let input = saved
                .get("file")
                .or_else(|| saved.values().next())
                .ok_or_else(|| anyhow!("no input file uploaded"))?;
            run_server_operation(input, regime, operation, out_dir)
        }
    }
}

fn run_emir_book_reconcile_server(
    book: &Path,
    tsr: &Path,
    mapping: &Path,
    out_dir: &Path,
) -> Result<ScanArtifacts> {
    use opendqi_core::dq::compute_book_reconcile_issues;
    use opendqi_io::{read_emir_csv, CsvMapping};
    use opendqi_xml::read_emir_tr_state_xml;

    let started_at = Utc::now();
    let map = CsvMapping::from_path(mapping)
        .with_context(|| format!("loading mapping {}", mapping.display()))?;
    let book_records = read_emir_csv(book, &map)
        .with_context(|| format!("reading book CSV {}", book.display()))?;
    let tsr_outcome =
        read_emir_tr_state_xml(tsr).with_context(|| format!("reading TSR {}", tsr.display()))?;

    let mut issues = tsr_outcome.issues;
    issues.extend(compute_book_reconcile_issues(
        &book_records,
        &tsr_outcome.records,
    ));
    let ctx = CheckContext {
        thresholds: Thresholds::default(),
        today: Utc::now().date_naive(),
        now: Utc::now(),
    };
    finalize_issues(&mut issues, &ctx);
    finalize_artifacts(
        out_dir,
        Regime::Emir,
        UiRegime::Emir,
        book_records.len() as u32,
        &issues,
        &[
            book.to_string_lossy().into_owned(),
            tsr.to_string_lossy().into_owned(),
        ],
        started_at,
        Utc::now(),
    )
}

fn run_sftr_book_reconcile_server(
    book: &Path,
    tsr: &Path,
    mapping: &Path,
    out_dir: &Path,
) -> Result<ScanArtifacts> {
    use opendqi_core::dq::compute_sftr_book_reconcile_issues;
    use opendqi_io::{read_sftr_csv, CsvMapping};
    use opendqi_xml::read_sftr_tr_state_xml;

    let started_at = Utc::now();
    let map = CsvMapping::from_path(mapping)
        .with_context(|| format!("loading mapping {}", mapping.display()))?;
    let book_records = read_sftr_csv(book, &map)
        .with_context(|| format!("reading SFT book CSV {}", book.display()))?;
    let tsr_outcome = read_sftr_tr_state_xml(tsr)
        .with_context(|| format!("reading SFTR TSR {}", tsr.display()))?;

    let mut issues = tsr_outcome.issues;
    issues.extend(compute_sftr_book_reconcile_issues(
        &book_records,
        &tsr_outcome.records,
    ));
    let ctx = CheckContext {
        thresholds: Thresholds::default(),
        today: Utc::now().date_naive(),
        now: Utc::now(),
    };
    finalize_issues(&mut issues, &ctx);
    finalize_artifacts(
        out_dir,
        Regime::Sftr,
        UiRegime::Sftr,
        book_records.len() as u32,
        &issues,
        &[
            book.to_string_lossy().into_owned(),
            tsr.to_string_lossy().into_owned(),
        ],
        started_at,
        Utc::now(),
    )
}

fn run_emir_tr_audit_server(
    tar: &Path,
    tsr: &Path,
    feedback: &Path,
    out_dir: &Path,
) -> Result<ScanArtifacts> {
    use opendqi_core::dq::compute_tr_audit_emir_issues;

    let started_at = Utc::now();
    let tar_outcome =
        read_emir_xml(tar).with_context(|| format!("reading TAR {}", tar.display()))?;
    let tsr_outcome =
        read_emir_tr_state_xml(tsr).with_context(|| format!("reading TSR {}", tsr.display()))?;
    let fb_outcome = read_emir_feedback_xml(feedback)
        .with_context(|| format!("reading feedback {}", feedback.display()))?;

    let ctx = CheckContext {
        thresholds: Thresholds::default(),
        today: Utc::now().date_naive(),
        now: Utc::now(),
    };
    let mut issues = tar_outcome.issues;
    issues.extend(tsr_outcome.issues.clone());
    issues.extend(fb_outcome.issues.clone());
    issues.extend(run_all(&default_checks(), &tar_outcome.records, &ctx));
    issues.extend(run_all_tr_state(
        &default_tr_state_checks(),
        &tsr_outcome.records,
        &[],
        &ctx,
    ));
    issues.extend(run_all_feedback(
        &default_feedback_checks(),
        &fb_outcome.records,
        &[],
        &ctx,
    ));
    issues.extend(run_all_tr_activity(
        &default_tr_activity_checks(),
        &tar_outcome.records,
        &[],
        Some(&tsr_outcome.records),
        &ctx,
    ));
    issues.extend(compute_tr_audit_emir_issues(
        &tar_outcome.records,
        &tsr_outcome.records,
        &fb_outcome.records,
        &tsr.to_string_lossy(),
        &feedback.to_string_lossy(),
    ));
    finalize_issues(&mut issues, &ctx);
    finalize_artifacts(
        out_dir,
        Regime::Emir,
        UiRegime::Emir,
        tar_outcome.records.len() as u32,
        &issues,
        &[
            tar.to_string_lossy().into_owned(),
            tsr.to_string_lossy().into_owned(),
            feedback.to_string_lossy().into_owned(),
        ],
        started_at,
        Utc::now(),
    )
}

fn run_sftr_tr_audit_server(tar: &Path, tsr: &Path, out_dir: &Path) -> Result<ScanArtifacts> {
    use opendqi_core::dq::compute_tr_audit_sftr_issues;
    use opendqi_xml::read_sftr_tr_state_xml;

    let started_at = Utc::now();
    let tar_outcome =
        read_sftr_xml(tar).with_context(|| format!("reading SFTR TAR {}", tar.display()))?;
    let tsr_outcome = read_sftr_tr_state_xml(tsr)
        .with_context(|| format!("reading SFTR TSR {}", tsr.display()))?;

    let ctx = CheckContext {
        thresholds: Thresholds::default(),
        today: Utc::now().date_naive(),
        now: Utc::now(),
    };
    let mut issues = tar_outcome.issues;
    issues.extend(tsr_outcome.issues.clone());
    issues.extend(run_all_sftr(
        &default_sftr_checks(),
        &tar_outcome.records,
        &ctx,
    ));
    issues.extend(run_all_sftr_tr_state(
        &default_sftr_tr_state_checks(),
        &tsr_outcome.records,
        &[],
        &ctx,
    ));
    issues.extend(run_all_sftr_tr_activity(
        &default_sftr_tr_activity_checks(),
        &tar_outcome.records,
        &[],
        Some(&tsr_outcome.records),
        &ctx,
    ));
    issues.extend(compute_tr_audit_sftr_issues(
        &tar_outcome.records,
        &tsr_outcome.records,
        &tsr.to_string_lossy(),
    ));
    finalize_issues(&mut issues, &ctx);
    finalize_artifacts(
        out_dir,
        Regime::Sftr,
        UiRegime::Sftr,
        tar_outcome.records.len() as u32,
        &issues,
        &[
            tar.to_string_lossy().into_owned(),
            tsr.to_string_lossy().into_owned(),
        ],
        started_at,
        Utc::now(),
    )
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
            finalize_issues(&mut issues, &ctx);
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
            finalize_issues(&mut issues, &ctx);
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

/// EMIR Trade State Report (auth.107) scan — server-side.
fn run_emir_tr_state_server(input: &Path, out_dir: &Path) -> Result<ScanArtifacts> {
    if !has_extension(input, "xml") {
        return Err(anyhow!(
            "tr-state-scan expects an XML input (auth.107). Got {}",
            input.display()
        ));
    }
    let started_at = Utc::now();
    let outcome = read_emir_tr_state_xml(input)
        .with_context(|| format!("reading TSR file {}", input.display()))?;
    let now = Utc::now();
    let ctx = CheckContext {
        thresholds: Thresholds::default(),
        today: now.date_naive(),
        now,
    };
    let mut issues = outcome.issues;
    issues.extend(run_all_tr_state(
        &default_tr_state_checks(),
        &outcome.records,
        &[],
        &ctx,
    ));
    finalize_issues(&mut issues, &ctx);
    finalize_artifacts(
        out_dir,
        Regime::Emir,
        UiRegime::Emir,
        outcome.records.len() as u32,
        &issues,
        &[input.to_string_lossy().into_owned()],
        started_at,
        Utc::now(),
    )
}

/// SFTR Trade State Report (auth.079) scan — server-side.
fn run_sftr_tr_state_server(input: &Path, out_dir: &Path) -> Result<ScanArtifacts> {
    if !has_extension(input, "xml") {
        return Err(anyhow!(
            "tr-state-scan expects an XML input (auth.079). Got {}",
            input.display()
        ));
    }
    let started_at = Utc::now();
    let outcome = read_sftr_tr_state_xml(input)
        .with_context(|| format!("reading SFTR TSR file {}", input.display()))?;
    let now = Utc::now();
    let ctx = CheckContext {
        thresholds: Thresholds::default(),
        today: now.date_naive(),
        now,
    };
    let mut issues = outcome.issues;
    issues.extend(run_all_sftr_tr_state(
        &default_sftr_tr_state_checks(),
        &outcome.records,
        &[],
        &ctx,
    ));
    finalize_issues(&mut issues, &ctx);
    finalize_artifacts(
        out_dir,
        Regime::Sftr,
        UiRegime::Sftr,
        outcome.records.len() as u32,
        &issues,
        &[input.to_string_lossy().into_owned()],
        started_at,
        Utc::now(),
    )
}

/// EMIR Trade Activity Report (auth.030 TR replay) scan — server-side.
fn run_emir_tr_activity_server(input: &Path, out_dir: &Path) -> Result<ScanArtifacts> {
    if !has_extension(input, "xml") {
        return Err(anyhow!(
            "tr-activity-scan expects an XML input (auth.030). Got {}",
            input.display()
        ));
    }
    let started_at = Utc::now();
    let outcome = read_emir_xml(input)
        .with_context(|| format!("reading EMIR TAR file {}", input.display()))?;
    let now = Utc::now();
    let ctx = CheckContext {
        thresholds: Thresholds::default(),
        today: now.date_naive(),
        now,
    };
    let mut issues = outcome.issues;
    issues.extend(run_all_tr_activity(
        &default_tr_activity_checks(),
        &outcome.records,
        &[],
        None,
        &ctx,
    ));
    finalize_issues(&mut issues, &ctx);
    finalize_artifacts(
        out_dir,
        Regime::Emir,
        UiRegime::Emir,
        outcome.records.len() as u32,
        &issues,
        &[input.to_string_lossy().into_owned()],
        started_at,
        Utc::now(),
    )
}

/// SFTR Trade Activity Report (auth.052 TR replay) scan — server-side.
fn run_sftr_tr_activity_server(input: &Path, out_dir: &Path) -> Result<ScanArtifacts> {
    if !has_extension(input, "xml") {
        return Err(anyhow!(
            "tr-activity-scan expects an XML input (auth.052). Got {}",
            input.display()
        ));
    }
    let started_at = Utc::now();
    let outcome = read_sftr_xml(input)
        .with_context(|| format!("reading SFTR TAR file {}", input.display()))?;
    let now = Utc::now();
    let ctx = CheckContext {
        thresholds: Thresholds::default(),
        today: now.date_naive(),
        now,
    };
    let mut issues = outcome.issues;
    issues.extend(run_all_sftr_tr_activity(
        &default_sftr_tr_activity_checks(),
        &outcome.records,
        &[],
        None,
        &ctx,
    ));
    finalize_issues(&mut issues, &ctx);
    finalize_artifacts(
        out_dir,
        Regime::Sftr,
        UiRegime::Sftr,
        outcome.records.len() as u32,
        &issues,
        &[input.to_string_lossy().into_owned()],
        started_at,
        Utc::now(),
    )
}

/// EMIR feedback file scan (auth.092) — server-side. Without a store,
/// only the format / namespace issues fire; the store-cross-referenced
/// `EMIR.FBK.*` family remains CLI-only.
fn run_emir_feedback_server(input: &Path, out_dir: &Path) -> Result<ScanArtifacts> {
    if !has_extension(input, "xml") {
        return Err(anyhow!(
            "feedback expects an XML input (auth.092). Got {}",
            input.display()
        ));
    }
    let started_at = Utc::now();
    let outcome = read_emir_feedback_xml(input)
        .with_context(|| format!("reading EMIR feedback file {}", input.display()))?;
    let now = Utc::now();
    let ctx = CheckContext {
        thresholds: Thresholds::default(),
        today: now.date_naive(),
        now,
    };
    let mut issues = outcome.issues;
    issues.extend(run_all_feedback(
        &default_feedback_checks(),
        &outcome.records,
        &[],
        &ctx,
    ));
    finalize_issues(&mut issues, &ctx);
    finalize_artifacts(
        out_dir,
        Regime::Emir,
        UiRegime::Emir,
        outcome.records.len() as u32,
        &issues,
        &[input.to_string_lossy().into_owned()],
        started_at,
        Utc::now(),
    )
}

/// EMIR Margin Activity Report (auth.108) scan — server-side.
fn run_emir_mar_server(input: &Path, out_dir: &Path) -> Result<ScanArtifacts> {
    if !has_extension(input, "xml") {
        return Err(anyhow!(
            "mar-scan expects an XML input (auth.108). Got {}",
            input.display()
        ));
    }
    let started_at = Utc::now();
    let outcome = read_emir_mar_xml(input)
        .with_context(|| format!("reading MAR file {}", input.display()))?;
    let now = Utc::now();
    let ctx = CheckContext {
        thresholds: Thresholds::default(),
        today: now.date_naive(),
        now,
    };
    let mut issues = outcome.issues;
    issues.extend(run_all_margin_activity(
        &default_margin_activity_checks(),
        &outcome.records,
        &[],
        &ctx,
    ));
    finalize_issues(&mut issues, &ctx);
    finalize_artifacts(
        out_dir,
        Regime::Emir,
        UiRegime::Emir,
        outcome.records.len() as u32,
        &issues,
        &[input.to_string_lossy().into_owned()],
        started_at,
        Utc::now(),
    )
}

/// EMIR Margin State Report (auth.109) scan — server-side.
fn run_emir_msr_server(input: &Path, out_dir: &Path) -> Result<ScanArtifacts> {
    if !has_extension(input, "xml") {
        return Err(anyhow!(
            "msr-scan expects an XML input (auth.109). Got {}",
            input.display()
        ));
    }
    let started_at = Utc::now();
    let outcome = read_emir_msr_xml(input)
        .with_context(|| format!("reading MSR file {}", input.display()))?;
    let now = Utc::now();
    let ctx = CheckContext {
        thresholds: Thresholds::default(),
        today: now.date_naive(),
        now,
    };
    let mut issues = outcome.issues;
    issues.extend(run_all_margin_state(
        &default_margin_state_checks(),
        &outcome.records,
        &[],
        &ctx,
    ));
    finalize_issues(&mut issues, &ctx);
    finalize_artifacts(
        out_dir,
        Regime::Emir,
        UiRegime::Emir,
        outcome.records.len() as u32,
        &issues,
        &[input.to_string_lossy().into_owned()],
        started_at,
        Utc::now(),
    )
}

/// XML well-formedness validation (regime-agnostic). v1 doesn't
/// expose an XSD picker; the schema validation remains CLI-only.
/// Emits a single Info issue on success or a Critical
/// `EMIR.FMT.XML_NOT_WELLFORMED` (or SFTR equivalent) on failure.
fn run_validate_server(input: &Path, regime: UiRegime, out_dir: &Path) -> Result<ScanArtifacts> {
    if !has_extension(input, "xml") {
        return Err(anyhow!(
            "validate expects an XML input. Got {}",
            input.display()
        ));
    }
    let started_at = Utc::now();
    let (core_regime, prefix) = match regime {
        UiRegime::Emir => (Regime::Emir, "EMIR"),
        UiRegime::Sftr => (Regime::Sftr, "SFTR"),
    };
    let source_label = input.to_string_lossy().into_owned();
    let issue = match check_wellformedness(input) {
        Ok(()) => DqIssue {
            check_id: format!("{prefix}.FMT.XML_WELLFORMED"),
            regime: core_regime,
            severity: Severity::Info,
            dimension: DqDimension::Validity,
            record_id: None,
            uti: None,
            field: None,
            value: None,
            message: "XML is well-formed.".into(),
            source_file: Some(source_label.clone()),
            evidence: Vec::new(),
        },
        Err(err) => DqIssue {
            check_id: format!("{prefix}.FMT.XML_NOT_WELLFORMED"),
            regime: core_regime,
            severity: Severity::Critical,
            dimension: DqDimension::Validity,
            record_id: None,
            uti: None,
            field: None,
            value: None,
            message: format!("XML is not well-formed: {}", err.message),
            source_file: Some(source_label.clone()),
            evidence: Vec::new(),
        },
    };
    let issues = vec![issue];
    finalize_artifacts(
        out_dir,
        core_regime,
        regime,
        0,
        &issues,
        &[source_label],
        started_at,
        Utc::now(),
    )
}

/// EMIR Reconciliation Statistics (auth.091) scan — server-side.
fn run_emir_recon_stats_server(input: &Path, out_dir: &Path) -> Result<ScanArtifacts> {
    if !has_extension(input, "xml") {
        return Err(anyhow!(
            "recon-stats expects an XML input (auth.091). Got {}",
            input.display()
        ));
    }
    let started_at = Utc::now();
    let outcome = read_emir_recon_stats_xml(input)
        .with_context(|| format!("reading auth.091 file {}", input.display()))?;
    let now = Utc::now();
    let ctx = CheckContext {
        thresholds: Thresholds::default(),
        today: now.date_naive(),
        now,
    };
    let mut issues = outcome.issues;
    issues.extend(run_all_recon_stats(
        &default_recon_stats_checks(),
        &outcome.records,
        &[],
        &ctx,
    ));
    // Per-transaction RcncltnRpt detail → EMIR.REC.* (same fold as the
    // CLI `opendqi emir recon-stats`; shared core, no duplicated logic).
    issues.extend(run_all_reconciliation(
        &default_reconciliation_checks(),
        &outcome.reconciliation_records,
        &[],
        &ctx,
    ));
    finalize_issues(&mut issues, &ctx);
    finalize_artifacts(
        out_dir,
        Regime::Emir,
        UiRegime::Emir,
        outcome.records.len() as u32,
        &issues,
        &[input.to_string_lossy().into_owned()],
        started_at,
        Utc::now(),
    )
}

/// Shared post-processing: build a summary, write the three artifact
/// files, and collect ScanArtifacts. Keeps the new scan helpers tiny.
#[allow(clippy::too_many_arguments)]
fn finalize_artifacts(
    out_dir: &Path,
    regime: Regime,
    ui_regime: UiRegime,
    records: u32,
    issues: &[DqIssue],
    sources: &[String],
    started_at: chrono::DateTime<Utc>,
    finished_at: chrono::DateTime<Utc>,
) -> Result<ScanArtifacts> {
    let summary = build_summary(regime, records, issues, 1, started_at, finished_at);
    write_summary_json(&out_dir.join("summary.json"), &summary).context("writing summary.json")?;
    write_issues_csv(&out_dir.join("issues.csv"), issues).context("writing issues.csv")?;
    write_report_html(&out_dir.join("report.html"), &summary, issues, sources)
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
    Ok(ScanArtifacts {
        regime: ui_regime,
        records,
        issues_total: summary.issues_total,
        issues_critical: critical,
        issues_high: high,
        quality_score: summary.quality_score,
        artifact_files: vec![
            "report.html".to_string(),
            "summary.json".to_string(),
            "issues.csv".to_string(),
        ],
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
