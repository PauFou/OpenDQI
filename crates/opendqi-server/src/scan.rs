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
    default_margin_state_checks, default_missing_collateral_checks, default_recon_stats_checks,
    default_reconciliation_checks, default_sftr_checks, default_sftr_tr_activity_checks,
    default_sftr_tr_state_checks, default_tr_activity_checks, default_tr_state_checks,
    default_warnings_checks, default_warnings_counterparty_checks,
    default_warnings_transaction_checks, stream_checks_into, CheckContext,
};
use opendqi_core::{
    DqDimension, DqIssue, EmirRecord, Regime, Severity, SftrRecord, SortedIssueSink, Thresholds,
    STREAM_SPILL_MAX_ISSUES,
};
use opendqi_io::{has_extension, read_emir_parquet, read_sftr_parquet};
use opendqi_report::{
    write_issues_csv_from_iter, write_report_html, write_summary_json, TopIssues,
};
use opendqi_xml::{
    check_wellformedness, read_emir_feedback_xml, read_emir_mar_xml, read_emir_msr_xml,
    read_emir_recon_stats_xml, read_emir_tr_state_xml, read_emir_warnings_xml, read_emir_xml,
    read_sftr_missing_collateral_xml, read_sftr_tr_state_xml, read_sftr_xml,
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
    /// EMIR Data-Quality Warnings scan (auth.106). EMIR-only.
    Warnings,
    /// EMIR Margin Activity Report scan (auth.108). EMIR-only.
    MarScan,
    /// EMIR Margin State Report scan (auth.109). EMIR-only.
    MsrScan,
    /// SFTR Missing Collateral Request scan (auth.083). SFTR-only.
    MissingCollateral,
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
            "warnings" => Some(Self::Warnings),
            "mar-scan" | "mar_scan" => Some(Self::MarScan),
            "msr-scan" | "msr_scan" => Some(Self::MsrScan),
            "missing-collateral" | "missing_collateral" => Some(Self::MissingCollateral),
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
            Self::Warnings => "warnings",
            Self::MarScan => "mar-scan",
            Self::MsrScan => "msr-scan",
            Self::MissingCollateral => "missing-collateral",
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
        UiOperation::Warnings => match regime {
            UiRegime::Emir => run_emir_warnings_server(input, out_dir),
            UiRegime::Sftr => Err(anyhow!(
                "warnings is EMIR-only (auth.106); pick a different operation for SFTR."
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
        UiOperation::MissingCollateral => match regime {
            // Single-file path (no companion); the optional `auth.079`
            // TSR cross-ref is wired through `run_server_dispatch`.
            UiRegime::Sftr => run_sftr_missing_collateral_server(input, None, out_dir),
            UiRegime::Emir => Err(anyhow!(
                "missing-collateral is SFTR-only (auth.083); pick a different operation for EMIR."
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
        UiOperation::MissingCollateral => {
            // Optionally multi-file: the auth.083 `file` is required;
            // an `auth.079` `file_tsr` companion is optional (its
            // presence enables the 3 `SFTR.MCR.*` cross-ref checks).
            let input = saved.get("file").ok_or_else(|| {
                anyhow!("missing-collateral requires a `file` (auth.083 XML) upload")
            })?;
            let tsr = saved.get("file_tsr").map(PathBuf::as_path);
            match regime {
                UiRegime::Sftr => run_sftr_missing_collateral_server(input, tsr, out_dir),
                UiRegime::Emir => Err(anyhow!(
                    "missing-collateral is SFTR-only (auth.083); pick a different operation for EMIR."
                )),
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

    let ctx = CheckContext {
        thresholds: Thresholds::default(),
        today: Utc::now().date_naive(),
        now: Utc::now(),
    };
    let mut sink = SortedIssueSink::new(&ctx.thresholds, STREAM_SPILL_MAX_ISSUES);
    sink.push_batch(tsr_outcome.issues);
    sink.push_batch(compute_book_reconcile_issues(
        &book_records,
        &tsr_outcome.records,
    ));
    finalize_artifacts(
        out_dir,
        Regime::Emir,
        UiRegime::Emir,
        book_records.len() as u32,
        sink,
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

    let ctx = CheckContext {
        thresholds: Thresholds::default(),
        today: Utc::now().date_naive(),
        now: Utc::now(),
    };
    let mut sink = SortedIssueSink::new(&ctx.thresholds, STREAM_SPILL_MAX_ISSUES);
    sink.push_batch(tsr_outcome.issues);
    sink.push_batch(compute_sftr_book_reconcile_issues(
        &book_records,
        &tsr_outcome.records,
    ));
    finalize_artifacts(
        out_dir,
        Regime::Sftr,
        UiRegime::Sftr,
        book_records.len() as u32,
        sink,
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
    let sink = std::sync::Mutex::new(SortedIssueSink::new(
        &ctx.thresholds,
        STREAM_SPILL_MAX_ISSUES,
    ));
    {
        let mut s = sink.lock().expect("issue sink mutex");
        s.push_batch(tar_outcome.issues);
        s.push_batch(tsr_outcome.issues.clone());
        s.push_batch(fb_outcome.issues.clone());
    }
    stream_checks_into(&default_checks(), &sink, |c| {
        c.run(&tar_outcome.records, &ctx)
    });
    stream_checks_into(&default_tr_state_checks(), &sink, |c| {
        c.run(&tsr_outcome.records, &[], &ctx)
    });
    stream_checks_into(&default_feedback_checks(), &sink, |c| {
        c.run(&fb_outcome.records, &[], &ctx)
    });
    stream_checks_into(&default_tr_activity_checks(), &sink, |c| {
        c.run(&tar_outcome.records, &[], Some(&tsr_outcome.records), &ctx)
    });
    sink.lock()
        .expect("issue sink mutex")
        .push_batch(compute_tr_audit_emir_issues(
            &tar_outcome.records,
            &tsr_outcome.records,
            &fb_outcome.records,
            &tsr.to_string_lossy(),
            &feedback.to_string_lossy(),
        ));
    finalize_artifacts(
        out_dir,
        Regime::Emir,
        UiRegime::Emir,
        tar_outcome.records.len() as u32,
        sink.into_inner().expect("issue sink mutex"),
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
    let sink = std::sync::Mutex::new(SortedIssueSink::new(
        &ctx.thresholds,
        STREAM_SPILL_MAX_ISSUES,
    ));
    {
        let mut s = sink.lock().expect("issue sink mutex");
        s.push_batch(tar_outcome.issues);
        s.push_batch(tsr_outcome.issues.clone());
    }
    stream_checks_into(&default_sftr_checks(), &sink, |c| {
        c.run(&tar_outcome.records, &ctx)
    });
    stream_checks_into(&default_sftr_tr_state_checks(), &sink, |c| {
        c.run(&tsr_outcome.records, &[], &ctx)
    });
    stream_checks_into(&default_sftr_tr_activity_checks(), &sink, |c| {
        c.run(&tar_outcome.records, &[], Some(&tsr_outcome.records), &ctx)
    });
    sink.lock()
        .expect("issue sink mutex")
        .push_batch(compute_tr_audit_sftr_issues(
            &tar_outcome.records,
            &tsr_outcome.records,
            &tsr.to_string_lossy(),
        ));
    finalize_artifacts(
        out_dir,
        Regime::Sftr,
        UiRegime::Sftr,
        tar_outcome.records.len() as u32,
        sink.into_inner().expect("issue sink mutex"),
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

    // Streaming pipeline (Milestone 0.27): each regime arm streams its
    // checks into a spill-capable sink; the shared `finalize_artifacts`
    // chokepoint finishes it (files = 1) — byte-identical to the prior
    // run_all + finalize_issues + build_summary path for non-spilling
    // inputs.
    let (core_regime, records_processed, sink) = match regime {
        UiRegime::Emir => {
            let records = load_emir(input)?;
            let now = Utc::now();
            let ctx = CheckContext {
                thresholds: Thresholds::default(),
                today: now.date_naive(),
                now,
            };
            let sink = std::sync::Mutex::new(SortedIssueSink::new(
                &ctx.thresholds,
                STREAM_SPILL_MAX_ISSUES,
            ));
            stream_checks_into(&default_checks(), &sink, |c| c.run(&records, &ctx));
            (
                Regime::Emir,
                records.len() as u32,
                sink.into_inner().expect("issue sink mutex"),
            )
        }
        UiRegime::Sftr => {
            let records = load_sftr(input)?;
            let now = Utc::now();
            let ctx = CheckContext {
                thresholds: Thresholds::default(),
                today: now.date_naive(),
                now,
            };
            let sink = std::sync::Mutex::new(SortedIssueSink::new(
                &ctx.thresholds,
                STREAM_SPILL_MAX_ISSUES,
            ));
            stream_checks_into(&default_sftr_checks(), &sink, |c| c.run(&records, &ctx));
            (
                Regime::Sftr,
                records.len() as u32,
                sink.into_inner().expect("issue sink mutex"),
            )
        }
    };

    finalize_artifacts(
        out_dir,
        core_regime,
        regime,
        records_processed,
        sink,
        &sources,
        started_at,
        Utc::now(),
    )
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
    let sink = std::sync::Mutex::new(SortedIssueSink::new(
        &ctx.thresholds,
        STREAM_SPILL_MAX_ISSUES,
    ));
    sink.lock()
        .expect("issue sink mutex")
        .push_batch(outcome.issues);
    stream_checks_into(&default_tr_state_checks(), &sink, |c| {
        c.run(&outcome.records, &[], &ctx)
    });
    finalize_artifacts(
        out_dir,
        Regime::Emir,
        UiRegime::Emir,
        outcome.records.len() as u32,
        sink.into_inner().expect("issue sink mutex"),
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
    let sink = std::sync::Mutex::new(SortedIssueSink::new(
        &ctx.thresholds,
        STREAM_SPILL_MAX_ISSUES,
    ));
    sink.lock()
        .expect("issue sink mutex")
        .push_batch(outcome.issues);
    stream_checks_into(&default_sftr_tr_state_checks(), &sink, |c| {
        c.run(&outcome.records, &[], &ctx)
    });
    finalize_artifacts(
        out_dir,
        Regime::Sftr,
        UiRegime::Sftr,
        outcome.records.len() as u32,
        sink.into_inner().expect("issue sink mutex"),
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
    let sink = std::sync::Mutex::new(SortedIssueSink::new(
        &ctx.thresholds,
        STREAM_SPILL_MAX_ISSUES,
    ));
    sink.lock()
        .expect("issue sink mutex")
        .push_batch(outcome.issues);
    stream_checks_into(&default_tr_activity_checks(), &sink, |c| {
        c.run(&outcome.records, &[], None, &ctx)
    });
    finalize_artifacts(
        out_dir,
        Regime::Emir,
        UiRegime::Emir,
        outcome.records.len() as u32,
        sink.into_inner().expect("issue sink mutex"),
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
    let sink = std::sync::Mutex::new(SortedIssueSink::new(
        &ctx.thresholds,
        STREAM_SPILL_MAX_ISSUES,
    ));
    sink.lock()
        .expect("issue sink mutex")
        .push_batch(outcome.issues);
    stream_checks_into(&default_sftr_tr_activity_checks(), &sink, |c| {
        c.run(&outcome.records, &[], None, &ctx)
    });
    finalize_artifacts(
        out_dir,
        Regime::Sftr,
        UiRegime::Sftr,
        outcome.records.len() as u32,
        sink.into_inner().expect("issue sink mutex"),
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
    let sink = std::sync::Mutex::new(SortedIssueSink::new(
        &ctx.thresholds,
        STREAM_SPILL_MAX_ISSUES,
    ));
    sink.lock()
        .expect("issue sink mutex")
        .push_batch(outcome.issues);
    stream_checks_into(&default_feedback_checks(), &sink, |c| {
        c.run(&outcome.records, &[], &ctx)
    });
    finalize_artifacts(
        out_dir,
        Regime::Emir,
        UiRegime::Emir,
        outcome.records.len() as u32,
        sink.into_inner().expect("issue sink mutex"),
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
    let sink = std::sync::Mutex::new(SortedIssueSink::new(
        &ctx.thresholds,
        STREAM_SPILL_MAX_ISSUES,
    ));
    sink.lock()
        .expect("issue sink mutex")
        .push_batch(outcome.issues);
    stream_checks_into(&default_margin_activity_checks(), &sink, |c| {
        c.run(&outcome.records, &[], &ctx)
    });
    finalize_artifacts(
        out_dir,
        Regime::Emir,
        UiRegime::Emir,
        outcome.records.len() as u32,
        sink.into_inner().expect("issue sink mutex"),
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
    let sink = std::sync::Mutex::new(SortedIssueSink::new(
        &ctx.thresholds,
        STREAM_SPILL_MAX_ISSUES,
    ));
    sink.lock()
        .expect("issue sink mutex")
        .push_batch(outcome.issues);
    stream_checks_into(&default_margin_state_checks(), &sink, |c| {
        c.run(&outcome.records, &[], &ctx)
    });
    finalize_artifacts(
        out_dir,
        Regime::Emir,
        UiRegime::Emir,
        outcome.records.len() as u32,
        sink.into_inner().expect("issue sink mutex"),
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
    // Single well-formedness issue; no checks, no overrides (default
    // empty thresholds) — the sink's finish is byte-identical to the
    // former one-element write.
    let thresholds = Thresholds::default();
    let mut sink = SortedIssueSink::new(&thresholds, STREAM_SPILL_MAX_ISSUES);
    sink.push_batch(vec![issue]);
    finalize_artifacts(
        out_dir,
        core_regime,
        regime,
        0,
        sink,
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
    let sink = std::sync::Mutex::new(SortedIssueSink::new(
        &ctx.thresholds,
        STREAM_SPILL_MAX_ISSUES,
    ));
    sink.lock()
        .expect("issue sink mutex")
        .push_batch(outcome.issues);
    stream_checks_into(&default_recon_stats_checks(), &sink, |c| {
        c.run(&outcome.records, &[], &ctx)
    });
    // Per-transaction RcncltnRpt detail → EMIR.REC.* (same fold as the
    // CLI `opendqi emir recon-stats`; shared core, no duplicated logic).
    stream_checks_into(&default_reconciliation_checks(), &sink, |c| {
        c.run(&outcome.reconciliation_records, &[], &ctx)
    });
    finalize_artifacts(
        out_dir,
        Regime::Emir,
        UiRegime::Emir,
        outcome.records.len() as u32,
        sink.into_inner().expect("issue sink mutex"),
        &[input.to_string_lossy().into_owned()],
        started_at,
        Utc::now(),
    )
}

/// EMIR Data-Quality Warnings (auth.106) scan — server-side. Mirrors
/// the CLI `opendqi emir warnings`; shared core, no duplicated logic.
fn run_emir_warnings_server(input: &Path, out_dir: &Path) -> Result<ScanArtifacts> {
    if !has_extension(input, "xml") {
        return Err(anyhow!(
            "warnings expects an XML input (auth.106). Got {}",
            input.display()
        ));
    }
    let started_at = Utc::now();
    let outcome = read_emir_warnings_xml(input)
        .with_context(|| format!("reading auth.106 file {}", input.display()))?;
    let now = Utc::now();
    let ctx = CheckContext {
        thresholds: Thresholds::default(),
        today: now.date_naive(),
        now,
    };
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
    stream_checks_into(&default_warnings_counterparty_checks(), &sink, |c| {
        c.run(&outcome.counterparty_records, &ctx)
    });
    stream_checks_into(&default_warnings_transaction_checks(), &sink, |c| {
        c.run(&outcome.transaction_records, &ctx)
    });
    finalize_artifacts(
        out_dir,
        Regime::Emir,
        UiRegime::Emir,
        outcome.records.len() as u32,
        sink.into_inner().expect("issue sink mutex"),
        &[input.to_string_lossy().into_owned()],
        started_at,
        Utc::now(),
    )
}

/// SFTR Missing Collateral Request (auth.083) scan — server-side.
/// Mirrors the CLI `opendqi sftr missing-collateral`; shared core, no
/// duplicated logic.
fn run_sftr_missing_collateral_server(
    input: &Path,
    tsr: Option<&Path>,
    out_dir: &Path,
) -> Result<ScanArtifacts> {
    use opendqi_xml::read_sftr_tr_state_xml;

    if !has_extension(input, "xml") {
        return Err(anyhow!(
            "missing-collateral expects an XML input (auth.083). Got {}",
            input.display()
        ));
    }
    let started_at = Utc::now();
    let outcome = read_sftr_missing_collateral_xml(input)
        .with_context(|| format!("reading auth.083 file {}", input.display()))?;

    // Optional companion SFTR TSR (auth.079): when supplied, the
    // requested UTIs are cross-referenced against the firm's TR state
    // (the 3 `SFTR.MCR.*` cross-ref checks). Store-backed cross-ref
    // stays CLI-only — the web UI has no history store.
    let tsr_outcome = match tsr {
        Some(p) => Some(
            read_sftr_tr_state_xml(p)
                .with_context(|| format!("reading companion SFTR TSR {}", p.display()))?,
        ),
        None => None,
    };

    let now = Utc::now();
    let ctx = CheckContext {
        thresholds: Thresholds::default(),
        today: now.date_naive(),
        now,
    };
    let sink = std::sync::Mutex::new(SortedIssueSink::new(
        &ctx.thresholds,
        STREAM_SPILL_MAX_ISSUES,
    ));
    {
        let mut s = sink.lock().expect("issue sink mutex");
        s.push_batch(outcome.issues);
        if let Some(o) = &tsr_outcome {
            s.push_batch(o.issues.clone());
        }
    }
    stream_checks_into(&default_missing_collateral_checks(), &sink, |c| {
        c.run(
            &outcome.records,
            tsr_outcome.as_ref().map(|o| o.records.as_slice()),
            &ctx,
        )
    });

    let mut sources = vec![input.to_string_lossy().into_owned()];
    if let Some(p) = tsr {
        sources.push(p.to_string_lossy().into_owned());
    }
    finalize_artifacts(
        out_dir,
        Regime::Sftr,
        UiRegime::Sftr,
        outcome.records.len() as u32,
        sink.into_inner().expect("issue sink mutex"),
        &sources,
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
    sink: SortedIssueSink,
    sources: &[String],
    started_at: chrono::DateTime<Utc>,
    finished_at: chrono::DateTime<Utc>,
) -> Result<ScanArtifacts> {
    // Streaming finalize (Milestone 0.27): the sink owns the online
    // aggregator + spill-capable issue store; `finish` yields the
    // `ScanSummary` and issues in exact `issue_cmp` order. files = 1
    // (one uploaded file) — exactly the former
    // `build_summary(regime, records, &issues, 1, …)`. Small inputs
    // never spill ⇒ byte-identical to the prior finalize path.
    let (summary, sorted) = sink.finish(regime, 1, records, started_at, finished_at);
    write_summary_json(&out_dir.join("summary.json"), &summary).context("writing summary.json")?;
    let mut top = TopIssues::with_capacity(20);
    write_issues_csv_from_iter(
        &out_dir.join("issues.csv"),
        sorted.inspect(|issue| top.offer(issue)),
    )
    .context("writing issues.csv")?;
    let top = top.into_sorted();
    write_report_html(&out_dir.join("report.html"), &summary, &top, sources)
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
