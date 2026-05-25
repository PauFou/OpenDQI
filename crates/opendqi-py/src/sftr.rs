//! `opendqi.sftr` submodule — SFTR-side bindings (symmetric to `emir`).

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use chrono::Utc;
use pyo3::prelude::*;
use pyo3::types::PyAny;

use opendqi_core::dq::{
    compute_sftr_book_reconcile_issues, compute_tr_audit_sftr_issues,
    default_missing_collateral_checks, default_sftr_checks, default_sftr_tr_state_checks,
    stream_checks_into, CheckContext,
};
use opendqi_core::{
    compute_sftr_dqi_pack, MappingPresence, Regime, SftrDqiInputs, SftrRecord, SortedIssueSink,
    Thresholds,
};
use opendqi_io::CsvMapping;

use crate::convert::{batch_to_sftr_records, Mapping};
use crate::dqi_schemas::{evidence_to_record_batch, indicators_to_record_batch};
use crate::emir::{pyarrow_to_record_batch, record_batch_to_pyarrow_table};
use crate::errors::to_py_err;
use crate::issues::issues_to_record_batch;
use crate::result::{PyDqiPackResult, PyScanResult};

const STREAM_SPILL_MAX_ISSUES: usize = 65_536;

fn scan_sftr_records(
    records: Vec<SftrRecord>,
    files: u32,
    normalize: bool,
) -> PyResult<PyScanResult> {
    let started_at = Utc::now();
    let ctx = CheckContext::now_with_defaults();
    let checks = default_sftr_checks();
    let thresholds = Thresholds::default();
    let sink = Mutex::new(SortedIssueSink::new(&thresholds, STREAM_SPILL_MAX_ISSUES));
    // SFTR uses the generic `stream_checks_into` with a closure
    // that captures `&records` and `&ctx` (the EMIR-side
    // `stream_emir_checks_into` is just a single-line delegate
    // around this for the `Check` trait).
    stream_checks_into(&checks, &sink, |c| c.run(&records, &ctx));
    let finished_at = Utc::now();
    let n = records.len() as u32;
    let normalized_batch = if normalize {
        let schema = opendqi_io::sftr_schema();
        Some(opendqi_io::build_sftr_batch(&schema, &records).map_err(to_py_err)?)
    } else {
        None
    };
    let (summary, sorted_issues) = sink.into_inner().expect("sink mutex not poisoned").finish(
        Regime::Sftr,
        files,
        n,
        started_at,
        finished_at,
    );
    let batch = issues_to_record_batch(sorted_issues).map_err(to_py_err)?;
    Ok(PyScanResult::new(summary, Some(batch), normalized_batch))
}

/// `opendqi.sftr.scan_parquet(path: str) -> PyScanResult`.
#[pyfunction]
#[pyo3(signature = (path, *, normalize = false))]
pub fn scan_parquet(path: &str, normalize: bool) -> PyResult<PyScanResult> {
    let records = opendqi_io::read_sftr_parquet(Path::new(path)).map_err(to_py_err)?;
    scan_sftr_records(records, 1, normalize)
}

/// `opendqi.sftr.scan_table(table, mapping={...}) -> PyScanResult`.
/// Same contract as `opendqi.emir.scan_table` (see that function's
/// docstring for the mapping direction and type strictness).
#[pyfunction]
#[pyo3(signature = (table, mapping, *, normalize = false))]
pub fn scan_table<'py>(
    table: &Bound<'py, PyAny>,
    mapping: HashMap<String, String>,
    normalize: bool,
) -> PyResult<PyScanResult> {
    let batch = pyarrow_to_record_batch(table)?;
    let records = batch_to_sftr_records(&batch, &mapping as &Mapping).map_err(to_py_err)?;
    scan_sftr_records(records, 1, normalize)
}

/// `opendqi.sftr.parse_xml(path: str) -> pyarrow.Table`.
/// Parses a SFTR firm-submission ISO 20022 XML (`auth.052.001.02`)
/// into the canonical Arrow Table. Same pattern as
/// `opendqi.emir.parse_xml` — see that docstring.
#[pyfunction]
pub fn parse_xml<'py>(py: Python<'py>, path: &str) -> PyResult<Bound<'py, PyAny>> {
    let outcome = opendqi_xml::read_sftr_xml(Path::new(path)).map_err(to_py_err)?;
    let schema = opendqi_io::sftr_schema();
    let batch = opendqi_io::build_sftr_batch(&schema, &outcome.records).map_err(to_py_err)?;
    record_batch_to_pyarrow_table(py, &batch)
}

// =================================================================
// v0.13.0 — multi-file entry points (mirror of emir.rs)
// =================================================================

/// SFTR equivalent of `read_emir_path_as_records` — see EMIR
/// docstring for the rationale on the extension dispatch and
/// CSV rejection.
fn read_sftr_path_as_records(path: &Path) -> PyResult<Vec<SftrRecord>> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "xml" => Ok(opendqi_xml::read_sftr_xml(path).map_err(to_py_err)?.records),
        "parquet" => opendqi_io::read_sftr_parquet(path).map_err(to_py_err),
        "csv" => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "scan_directory / scan_files cannot handle CSV inputs in v0.13.0 \
             (CSV requires a column mapping). For CSV: use \
             `opendqi.sftr.scan_table(pyarrow.csv.read_csv(path), mapping=…)` \
             on each file. Offending path: {}",
            path.display()
        ))),
        other => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "unsupported file extension {:?} (expected .xml, .parquet, or .csv; \
             .csv requires a mapping and isn't supported by the multi-file \
             entry points). Offending path: {}",
            other,
            path.display()
        ))),
    }
}

/// `opendqi.sftr.scan_directory(path: str) -> PyScanResult`.
/// Symmetric to `opendqi.emir.scan_directory` — see that
/// docstring for the contract.
///
/// Internally reuses `opendqi_io::discover_emir_inputs` (the
/// same walker is used for both regimes: it filters by file
/// extension, not by regime).
#[pyfunction]
#[pyo3(signature = (path, *, normalize = false))]
pub fn scan_directory(path: &str, normalize: bool) -> PyResult<PyScanResult> {
    let paths = opendqi_io::discover_emir_inputs(Path::new(path)).map_err(to_py_err)?;
    scan_sftr_paths(&paths, normalize)
}

/// `opendqi.sftr.scan_files(paths: list[str]) -> PyScanResult`.
/// Symmetric to `opendqi.emir.scan_files`.
#[pyfunction]
#[pyo3(signature = (paths, *, normalize = false))]
pub fn scan_files(paths: Vec<String>, normalize: bool) -> PyResult<PyScanResult> {
    let path_bufs: Vec<std::path::PathBuf> = paths.iter().map(std::path::PathBuf::from).collect();
    scan_sftr_paths(&path_bufs, normalize)
}

/// Shared helper for `scan_directory` + `scan_files` (SFTR).
fn scan_sftr_paths(paths: &[std::path::PathBuf], normalize: bool) -> PyResult<PyScanResult> {
    if paths.is_empty() {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "scan_directory / scan_files received no input files (empty \
             directory or empty list). Provide at least one .xml or .parquet path.",
        ));
    }
    let mut all_records: Vec<SftrRecord> = Vec::new();
    for p in paths {
        let recs = read_sftr_path_as_records(p)?;
        all_records.extend(recs);
    }
    let files = paths.len() as u32;
    scan_sftr_records(all_records, files, normalize)
}

// =================================================================
// v0.13.0 — cross-message workflow: tr_audit (2-layer SFTR)
// =================================================================

/// `opendqi.sftr.tr_audit(*, tar, tsr) -> PyScanResult`.
///
/// 2-layer SFTR audit (SFTR has no rejection-feedback message —
/// the synthetic `SFTR.FBK.*` family was retired in M0.4 of the
/// Rust core; `auth.080` is a reconciliation status advice, not
/// feedback). Parses the TAR (`auth.052`) + TSR (`auth.079`),
/// runs each layer's check pack, then the 2 `SFTR.AUD.*`
/// cross-layer coherence checks (`NEWT_IN_TAR_NOT_IN_TSR`,
/// `OUTSTANDING_IN_TSR_NOT_IN_TAR`).
///
/// Mirror of `opendqi.emir.tr_audit` modulo the missing feedback
/// layer.
#[pyfunction]
#[pyo3(signature = (*, tar, tsr))]
pub fn tr_audit(tar: &str, tsr: &str) -> PyResult<PyScanResult> {
    let started_at = Utc::now();
    let ctx = CheckContext::now_with_defaults();
    let thresholds = Thresholds::default();

    let tar_records = opendqi_xml::read_sftr_xml(Path::new(tar))
        .map_err(to_py_err)?
        .records;
    let tsr_records = opendqi_xml::read_sftr_tr_state_xml(Path::new(tsr))
        .map_err(to_py_err)?
        .records;

    let sink = Mutex::new(SortedIssueSink::new(&thresholds, STREAM_SPILL_MAX_ISSUES));

    // Layer 1 — TAR records ⊗ default_sftr_checks.
    let tar_checks = default_sftr_checks();
    stream_checks_into(&tar_checks, &sink, |c| c.run(&tar_records, &ctx));

    // Layer 2 — TSR records ⊗ default_sftr_tr_state_checks.
    // The `prior` arg is history-store-loaded SftrRecord set —
    // Python doesn't touch the store in v0.13, so we pass an
    // empty slice (lifecycle-flavored checks that need history
    // simply produce no issues).
    let tsr_checks = default_sftr_tr_state_checks();
    let prior_sftr: &[SftrRecord] = &[];
    stream_checks_into(&tsr_checks, &sink, |c| {
        c.run(&tsr_records, prior_sftr, &ctx)
    });

    // Cross-layer — the 2 SFTR.AUD.* checks.
    let cross = compute_tr_audit_sftr_issues(&tar_records, &tsr_records, tsr);
    if !cross.is_empty() {
        sink.lock()
            .expect("sink mutex not poisoned")
            .push_batch(cross);
    }

    let finished_at = Utc::now();
    let records_total = (tar_records.len() + tsr_records.len()) as u32;
    let (summary, sorted_issues) = sink.into_inner().expect("sink mutex not poisoned").finish(
        Regime::Sftr,
        2,
        records_total,
        started_at,
        finished_at,
    );
    let batch = issues_to_record_batch(sorted_issues).map_err(to_py_err)?;
    Ok(PyScanResult::new(summary, Some(batch), None))
}

// =================================================================
// v0.13.0 — cross-message workflow: missing_collateral
// =================================================================

/// `opendqi.sftr.missing_collateral(auth083, *, tsr=None) -> PyScanResult`.
///
/// Mirror of the CLI `opendqi sftr missing-collateral
/// auth083.xml [--tsr auth079.xml]`. Parses an SFTR Missing
/// Collateral Request (`auth.083`) and runs the 2 base
/// `SFTR.MCR.*` single-batch checks. If `tsr` is supplied, also
/// reads the SFTR Trade State Report (`auth.079`) and runs the
/// 3 additional cross-reference checks (`COLLATERAL_PRESENT_IN_TSR`,
/// `STILL_MISSING_IN_TSR`, `REQUESTED_UTI_NOT_IN_TSR`) — the
/// `tsr` cross-ref is the higher-value path most users want.
///
/// **Store-backed cross-ref** (the CLI's `--store` flag, which
/// looks up the latest persisted SFTR trade state for the
/// requested UTIs) is NOT exposed in v0.13.0 — store wrapping
/// is deferred to v0.14+. Users with a history store either use
/// the CLI or do their own SQL lookup and pass the TSR records
/// via the (unsupported in v0.13) `tsr` path argument.
#[pyfunction]
#[pyo3(signature = (auth083, *, tsr = None))]
pub fn missing_collateral(auth083: &str, tsr: Option<&str>) -> PyResult<PyScanResult> {
    let started_at = Utc::now();
    let ctx = CheckContext::now_with_defaults();
    let thresholds = Thresholds::default();

    let outcome =
        opendqi_xml::read_sftr_missing_collateral_xml(Path::new(auth083)).map_err(to_py_err)?;

    // Optional companion TSR for the 3 cross-ref checks. The
    // store-backed variant from the CLI is deliberately out of
    // scope (Python doesn't touch the SQLite store in v0.13).
    let tsr_records: Option<Vec<opendqi_core::SftrTrStateRecord>> = match tsr {
        Some(p) => Some(
            opendqi_xml::read_sftr_tr_state_xml(Path::new(p))
                .map_err(to_py_err)?
                .records,
        ),
        None => None,
    };

    let sink = Mutex::new(SortedIssueSink::new(&thresholds, STREAM_SPILL_MAX_ISSUES));
    // Format-level issues from the parser (file-not-well-formed,
    // unexpected namespace, ...) get included — they're a real
    // signal users want to see ; consistent with the CLI which
    // also `push_batch`es them.
    if !outcome.issues.is_empty() {
        sink.lock()
            .expect("sink mutex not poisoned")
            .push_batch(outcome.issues);
    }
    stream_checks_into(&default_missing_collateral_checks(), &sink, |c| {
        c.run(&outcome.records, tsr_records.as_deref(), &ctx)
    });

    let finished_at = Utc::now();
    let n = outcome.records.len() as u32;
    let files = if tsr.is_some() { 2 } else { 1 };
    let (summary, sorted_issues) = sink.into_inner().expect("sink mutex not poisoned").finish(
        Regime::Sftr,
        files,
        n,
        started_at,
        finished_at,
    );
    let batch = issues_to_record_batch(sorted_issues).map_err(to_py_err)?;
    Ok(PyScanResult::new(summary, Some(batch), None))
}

// =================================================================
// v0.13.0 — cross-message workflow: book_reconcile (SFTR)
// =================================================================

/// `opendqi.sftr.book_reconcile(book, tsr, *, mapping=None) -> PyScanResult`.
/// Symmetric to `opendqi.emir.book_reconcile`. See the EMIR
/// docstring for the dual-input (str path | pyarrow.Table)
/// contract.
///
/// Fires the 5 `SFTR.BREC.*` checks (LOAN_MISMATCH,
/// LOAN_CURRENCY_MISMATCH, COLLATERAL_MISMATCH,
/// MATURITY_MISMATCH, STATUS_MISMATCH).
///
/// v0.14.0: `date_format` and `datetime_format` kwargs override
/// the CSV parser defaults (`%Y-%m-%d` + RFC 3339). Ignored
/// when `book` is a pyarrow.Table/RecordBatch.
#[pyfunction]
#[pyo3(signature = (book, tsr, *, mapping = None, date_format = None, datetime_format = None))]
pub fn book_reconcile<'py>(
    book: &Bound<'py, PyAny>,
    tsr: &str,
    mapping: Option<HashMap<String, String>>,
    date_format: Option<String>,
    datetime_format: Option<String>,
) -> PyResult<PyScanResult> {
    let started_at = Utc::now();

    let tsr_records = opendqi_xml::read_sftr_tr_state_xml(Path::new(tsr))
        .map_err(to_py_err)?
        .records;

    let book_records: Vec<SftrRecord> = if let Ok(path_str) = book.extract::<String>() {
        let path = Path::new(&path_str);
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match ext.as_str() {
            "csv" => {
                let mapping = mapping.ok_or_else(|| {
                    pyo3::exceptions::PyValueError::new_err(
                        "book_reconcile with a .csv path requires `mapping={...}`.",
                    )
                })?;
                let csv_mapping = CsvMapping {
                    fields: mapping.into_iter().collect(),
                    date_format: date_format.clone(),
                    datetime_format: datetime_format.clone(),
                };
                opendqi_io::read_sftr_csv(path, &csv_mapping).map_err(to_py_err)?
            }
            "parquet" => opendqi_io::read_sftr_parquet(path).map_err(to_py_err)?,
            other => {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "book_reconcile: unsupported book file extension {:?} \
                     (expected .csv or .parquet). Path: {}",
                    other,
                    path.display()
                )));
            }
        }
    } else {
        let mapping = mapping.ok_or_else(|| {
            pyo3::exceptions::PyValueError::new_err(
                "book_reconcile with a pyarrow.Table/RecordBatch requires \
                 `mapping={...}`.",
            )
        })?;
        let batch = pyarrow_to_record_batch(book)?;
        crate::convert::batch_to_sftr_records(&batch, &mapping as &Mapping).map_err(to_py_err)?
    };

    let cross = compute_sftr_book_reconcile_issues(&book_records, &tsr_records);

    let thresholds = Thresholds::default();
    let sink = Mutex::new(SortedIssueSink::new(&thresholds, STREAM_SPILL_MAX_ISSUES));
    if !cross.is_empty() {
        sink.lock()
            .expect("sink mutex not poisoned")
            .push_batch(cross);
    }

    let finished_at = Utc::now();
    let records_total = (book_records.len() + tsr_records.len()) as u32;
    let (summary, sorted_issues) = sink.into_inner().expect("sink mutex not poisoned").finish(
        Regime::Sftr,
        2,
        records_total,
        started_at,
        finished_at,
    );
    let batch = issues_to_record_batch(sorted_issues).map_err(to_py_err)?;
    Ok(PyScanResult::new(summary, Some(batch), None))
}

// =================================================================
// v0.16.0 — SFTR Data Quality Pack
// =================================================================

/// `opendqi.sftr.data_quality_pack(*, tsr=None, tar=None,
/// reconciliation=None, missing_collateral=None, msr=None,
/// as_of=None) -> PyDqiPackResult`.
///
/// SFTR Data Quality Pack (v0.17): aggregates the 5 SFTR TR
/// layers into 16 regulator-style indicators (numerator /
/// denominator / rate / threshold / status) + ≤ 20 evidence
/// rows per indicator, AND co-produces the granular issue
/// stream from the existing SFTR check registries plus the
/// new SFTR.T3.* registry (F1'). All inputs are optional ; at
/// least one must be provided.
///
/// **v0.17 indicators by source layer:**
/// - TSR (`auth.079`) — 6 DQIs: LOAN_VALUE_MISSING/STALE,
///   COLLATERAL_VALUE_MISSING, HAIRCUT_ANOMALY, LEI_MISSING,
///   UNDER_COLLATERALIZATION (all _SFTR)
/// - TAR (`auth.052`) — 1 DQI: TIM_REPORTING_LATE_SFTR
/// - reconciliation (`auth.080`) — 4 DQIs: PAIRING_RATE,
///   RECONCILIATION_RATE, UNPAIRED_TRADES_RATE,
///   FIELD_MISMATCH_RATE (all _SFTR)
/// - missing_collateral (`auth.083`) — 1 DQI:
///   MCR_OPEN_REQUESTS_SFTR (best paired with tsr= for the
///   portfolio cross-ref; otherwise degraded mode)
/// - MSR (`auth.085`) — 4 T3 DQIs: T3_MARGIN_POSTED_MISSING,
///   T3_MARGIN_RECEIVED_MISSING, T3_EXCESS_COLLATERAL_USE,
///   T3_MARGIN_STALE (all _SFTR). MSR also triggers the 6
///   SFTR.T3.* per-record granular checks.
///
/// **Inputs (v0.17): paths-only.** All five accept a `str`
/// file path (XML) only. Dual-input pyarrow.Table path on
/// the SFTR side is scheduled for v0.18 (Arrow converters
/// for SftrTrStateRecord / SftrMarginStateRecord /
/// ReconciliationRecord / MissingCollateralRecord are not
/// yet implemented).
///
/// `as_of` defaults to today (UTC). Format: `YYYY-MM-DD`.
/// Pinning it keeps the stale-data cutoffs stable across
/// calendar days (same trick as the CLI golden).
///
/// Granular SFTR issues run on each provided layer using the
/// existing `default_sftr_checks` (TAR) +
/// `default_sftr_tr_state_checks` (TSR) registries, plus the
/// new `default_sftr_msr_checks` (MSR — F1'); `prior` is the
/// empty slice (lifecycle-needing checks no-op without a
/// history store — same as the v0.13 SFTR `tr_audit`
/// binding).
#[pyfunction]
#[pyo3(signature = (*, tsr = None, tar = None, reconciliation = None, missing_collateral = None, msr = None, mar = None, reuse_activity = None, reuse_state = None, as_of = None))]
pub fn data_quality_pack(
    tsr: Option<&str>,
    tar: Option<&str>,
    reconciliation: Option<&str>,
    missing_collateral: Option<&str>,
    msr: Option<&str>,
    mar: Option<&str>,
    reuse_activity: Option<&str>,
    reuse_state: Option<&str>,
    as_of: Option<&str>,
) -> PyResult<PyDqiPackResult> {
    if tsr.is_none()
        && tar.is_none()
        && reconciliation.is_none()
        && missing_collateral.is_none()
        && msr.is_none()
        && mar.is_none()
        && reuse_activity.is_none()
        && reuse_state.is_none()
    {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "data_quality_pack: at least one of tsr / tar / reconciliation / \
             missing_collateral / msr / mar / reuse_activity / reuse_state is required",
        ));
    }

    let thresholds = Thresholds::default();
    let as_of_date = match as_of {
        Some(s) => chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|e| {
            pyo3::exceptions::PyValueError::new_err(format!(
                "parsing as_of {s:?} (expected YYYY-MM-DD): {e}"
            ))
        })?,
        None => Utc::now().date_naive(),
    };

    // Load each provided layer (paths-only in v0.17).
    let tsr_records = match tsr {
        Some(p) => {
            opendqi_xml::read_sftr_tr_state_xml(Path::new(p))
                .map_err(to_py_err)?
                .records
        }
        None => Vec::new(),
    };
    let tar_records = match tar {
        Some(p) => {
            opendqi_xml::read_sftr_xml(Path::new(p))
                .map_err(to_py_err)?
                .records
        }
        None => Vec::new(),
    };
    let reconciliation_records = match reconciliation {
        Some(p) => {
            opendqi_xml::read_sftr_reconciliation_xml(Path::new(p))
                .map_err(to_py_err)?
                .records
        }
        None => Vec::new(),
    };
    let missing_collateral_records = match missing_collateral {
        Some(p) => {
            opendqi_xml::read_sftr_missing_collateral_xml(Path::new(p))
                .map_err(to_py_err)?
                .records
        }
        None => Vec::new(),
    };
    let msr_records = match msr {
        Some(p) => {
            opendqi_xml::read_sftr_margin_state_xml(Path::new(p))
                .map_err(to_py_err)?
                .records
        }
        None => Vec::new(),
    };
    let mar_records = match mar {
        Some(p) => {
            opendqi_xml::read_sftr_margin_activity_xml(Path::new(p))
                .map_err(to_py_err)?
                .records
        }
        None => Vec::new(),
    };
    let reuse_activity_records = match reuse_activity {
        Some(p) => {
            opendqi_xml::read_sftr_reuse_activity_xml(Path::new(p))
                .map_err(to_py_err)?
                .records
        }
        None => Vec::new(),
    };
    let reuse_state_records = match reuse_state {
        Some(p) => {
            opendqi_xml::read_sftr_reuse_state_xml(Path::new(p))
                .map_err(to_py_err)?
                .records
        }
        None => Vec::new(),
    };

    let inputs = SftrDqiInputs {
        tsr: tsr.map(|_| tsr_records.as_slice()),
        tar: tar.map(|_| tar_records.as_slice()),
        reconciliation: reconciliation.map(|_| reconciliation_records.as_slice()),
        missing_collateral: missing_collateral.map(|_| missing_collateral_records.as_slice()),
        msr: msr.map(|_| msr_records.as_slice()),
        // v0.18 A6: mar slot wired through `mar=` kwarg.
        mar: mar.map(|_| mar_records.as_slice()),
        // v0.18 B5: reuse_activity slot wired through
        // `reuse_activity=` kwarg.
        reuse_activity: reuse_activity.map(|_| reuse_activity_records.as_slice()),
        // v0.18 C5: reuse_state slot wired through
        // `reuse_state=` kwarg.
        reuse_state: reuse_state.map(|_| reuse_state_records.as_slice()),
    };

    let pack = compute_sftr_dqi_pack(inputs, MappingPresence::default(), &thresholds, as_of_date);

    let indicators_batch = indicators_to_record_batch(&pack.indicators).map_err(to_py_err)?;
    let evidence_batch = evidence_to_record_batch(&pack.evidence).map_err(to_py_err)?;
    let issues_batch = issues_to_record_batch(pack.issues.iter().cloned()).map_err(to_py_err)?;

    Ok(PyDqiPackResult::new(
        pack.indicators,
        pack.evidence,
        pack.issues,
        pack.issues_summary,
        indicators_batch,
        evidence_batch,
        issues_batch,
    ))
}

// =================================================================
// Module registration
// =================================================================

/// Register the `opendqi.sftr` submodule on the parent
/// `opendqi` module.
pub fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = parent.py();
    let m = PyModule::new_bound(py, "sftr")?;
    m.add("__doc__", "SFTR data-quality bindings.")?;
    // Single-file entry points (v0.12.0).
    m.add_function(wrap_pyfunction!(scan_parquet, &m)?)?;
    m.add_function(wrap_pyfunction!(scan_table, &m)?)?;
    m.add_function(wrap_pyfunction!(parse_xml, &m)?)?;
    // Multi-file entry points (v0.13.0).
    m.add_function(wrap_pyfunction!(scan_directory, &m)?)?;
    m.add_function(wrap_pyfunction!(scan_files, &m)?)?;
    // Cross-message workflow entry points (v0.13.0).
    m.add_function(wrap_pyfunction!(tr_audit, &m)?)?;
    m.add_function(wrap_pyfunction!(missing_collateral, &m)?)?;
    m.add_function(wrap_pyfunction!(book_reconcile, &m)?)?;
    // Data Quality Pack (v0.16.0).
    m.add_function(wrap_pyfunction!(data_quality_pack, &m)?)?;
    parent.add_submodule(&m)?;
    Ok(())
}
