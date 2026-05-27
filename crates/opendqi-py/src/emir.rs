//! `opendqi.emir` submodule — EMIR-side bindings.
//!
//! Single-file entry points (v0.12.0 P2-P4) :
//! - `scan_parquet(path)`        — scan a normalized EMIR Parquet.
//! - `scan_table(table, mapping)` — scan an Arrow table with a
//!                                  canonical-field → user-col
//!                                  mapping (avoids Parquet
//!                                  roundtrip).
//! - `parse_xml(path)`           — parse EMIR ISO 20022 firm
//!                                  submission XML into the
//!                                  canonical Arrow Table.
//!
//! Multi-file entry points (v0.13.0) :
//! - `scan_directory(path)`      — walk a directory of EMIR
//!                                  submissions, aggregate, scan.
//! - `scan_files([paths])`       — same with an explicit path list.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use arrow::array::RecordBatch;
use arrow::pyarrow::{FromPyArrow, ToPyArrow};
use chrono::Utc;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyList};

use opendqi_core::dq::{
    compute_book_reconcile_issues, compute_collateral_emir_issues, compute_tr_audit_emir_issues,
    default_checks, default_feedback_checks, default_margin_activity_checks,
    default_margin_state_checks, default_recon_stats_checks, default_tr_state_checks,
    default_warnings_checks, stream_checks_into, stream_emir_checks_into, CheckContext,
};
use opendqi_core::{
    compute_emir_dqi_pack, default_emir_query_checks, default_emir_status_advice_checks,
    EmirDqiInputs, EmirRecord, MappingPresence, Regime, SortedIssueSink, Thresholds,
};
use opendqi_io::CsvMapping;

use crate::convert::{batch_to_emir_records, Mapping};
use crate::dqi_schemas::{evidence_to_record_batch, indicators_to_record_batch};
use crate::errors::to_py_err;
use crate::issues::issues_to_record_batch;
use crate::result::{PyDqiPackResult, PyScanResult};

/// Match the CLI default — small enough that every shipped golden
/// stays in the no-spill path, which is byte-identical to the
/// legacy `finalize_issues` (M0.22 invariant).
const STREAM_SPILL_MAX_ISSUES: usize = 65_536;

/// Run the EMIR check suite against `records` and assemble the
/// `PyScanResult`. Shared by every EMIR entry point so the
/// behaviour stays identical across `scan_parquet`, `scan_table`,
/// and any future `scan_*` adder. If `normalize` is set, the
/// records are also projected back into the canonical Arrow
/// batch and exposed via `result.normalized` — same `RecordBatch`
/// the Parquet writer would produce.
fn scan_emir_records(
    records: Vec<EmirRecord>,
    files: u32,
    normalize: bool,
) -> PyResult<PyScanResult> {
    let started_at = Utc::now();
    let ctx = CheckContext::now_with_defaults();
    let checks = default_checks();
    let thresholds = Thresholds::default();
    let sink = Mutex::new(SortedIssueSink::new(&thresholds, STREAM_SPILL_MAX_ISSUES));
    stream_emir_checks_into(&checks, &records, &ctx, &sink);
    let finished_at = Utc::now();
    let n = records.len() as u32;

    // Optional `normalized` projection BEFORE the records are
    // moved into `finish`-side state — uses the public
    // `opendqi_io::{emir_schema, build_emir_batch}` exposed in P0.
    let normalized_batch = if normalize {
        let schema = opendqi_io::emir_schema();
        Some(opendqi_io::build_emir_batch(&schema, &records).map_err(to_py_err)?)
    } else {
        None
    };

    let (summary, sorted_issues) = sink.into_inner().expect("sink mutex not poisoned").finish(
        Regime::Emir,
        files,
        n,
        started_at,
        finished_at,
    );
    let batch = issues_to_record_batch(sorted_issues).map_err(to_py_err)?;
    Ok(PyScanResult::new(summary, Some(batch), normalized_batch))
}

/// `opendqi.emir.scan_parquet(path: str) -> PyScanResult`.
///
/// Reads the canonical EMIR Parquet at `path` (same schema as
/// `opendqi emir normalize --out`), runs the 89 single-batch EMIR
/// data-quality checks (`default_checks()`), and returns a
/// `PyScanResult` matching the on-disk `summary.json` shape +
/// the v1.0 stable Arrow schema for issues.
#[pyfunction]
#[pyo3(signature = (path, *, normalize = false))]
pub fn scan_parquet(path: &str, normalize: bool) -> PyResult<PyScanResult> {
    let records = opendqi_io::read_emir_parquet(Path::new(path)).map_err(to_py_err)?;
    scan_emir_records(records, 1, normalize)
}

/// `opendqi.emir.scan_table(table, mapping={...}) -> PyScanResult`.
///
/// Accepts a `pyarrow.Table` (or `pyarrow.RecordBatch`) and a
/// `mapping` dict of `canonical_field_name → user_column_name`.
/// The mapping direction matches the existing CSV mapping pattern
/// (`crates/opendqi-io/src/csv_in.rs:35-45`) — keys are the
/// canonical EMIR field names (e.g. `"uti"`, `"valuation_timestamp"`),
/// values are the names of the columns in the user's input.
///
/// Strict type contract: the mapped columns MUST already have the
/// canonical Arrow type (`Utf8`, `Decimal128(38,10)`, `Date32`,
/// `Timestamp(μs,UTC)`, `Boolean`). Users with string-only input
/// cast in Python first (`pa.compute.cast(col, pa.date32())`).
/// Unmapped canonical fields are emitted as `None` on every
/// record — downstream `EMIR.COMP.*` checks surface them.
#[pyfunction]
#[pyo3(signature = (table, mapping, *, normalize = false))]
pub fn scan_table<'py>(
    table: &Bound<'py, PyAny>,
    mapping: HashMap<String, String>,
    normalize: bool,
) -> PyResult<PyScanResult> {
    let batch = pyarrow_to_record_batch(table)?;
    let records = batch_to_emir_records(&batch, &mapping as &Mapping).map_err(to_py_err)?;
    scan_emir_records(records, 1, normalize)
}

/// `opendqi.emir.parse_xml(path: str) -> pyarrow.Table`.
///
/// Parses any EMIR firm-submission ISO 20022 XML (`auth.030.001.03`
/// or `auth.030.001.04`) into the canonical Arrow Table — the
/// same schema `opendqi emir normalize` produces. The output
/// can be passed directly into `scan_table` for a fast
/// XML → in-memory → checks pipeline that bypasses Parquet
/// entirely :
///
/// ```python
/// table = opendqi.emir.parse_xml("auth030.xml")
/// result = opendqi.emir.scan_table(table, mapping={
///     # identity mapping — the table already has canonical names
///     name: name for name in table.column_names
/// })
/// ```
#[pyfunction]
pub fn parse_xml<'py>(py: Python<'py>, path: &str) -> PyResult<Bound<'py, PyAny>> {
    let outcome = opendqi_xml::read_emir_xml(Path::new(path)).map_err(to_py_err)?;
    let schema = opendqi_io::emir_schema();
    let batch = opendqi_io::build_emir_batch(&schema, &outcome.records).map_err(to_py_err)?;
    record_batch_to_pyarrow_table(py, &batch)
}

// =================================================================
// v0.13.0 — multi-file entry points
// =================================================================

/// Dispatch a single path to the right EMIR reader by extension.
/// Returns the parsed records; format-level issues (file-not-
/// well-formed, wrong-namespace, ...) are dropped here — they
/// would surface as `EMIR.FMT.*` issues if we exposed them, but
/// `scan_directory` / `scan_files` are aggregator entry points
/// and we want clean errors for unreadable files, not in-band
/// issue mixing. A future v0.14 may add an `include_format_
/// issues=True` knob; for now we keep the v0.13 surface minimal.
fn read_emir_path_as_records(path: &Path) -> PyResult<Vec<EmirRecord>> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "xml" => Ok(opendqi_xml::read_emir_xml(path).map_err(to_py_err)?.records),
        "parquet" => opendqi_io::read_emir_parquet(path).map_err(to_py_err),
        "csv" => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "scan_directory / scan_files cannot handle CSV inputs in v0.13.0 \
             (CSV requires a column mapping). For CSV: use \
             `opendqi.emir.scan_table(pyarrow.csv.read_csv(path), mapping=…)` \
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

/// `opendqi.emir.scan_directory(path: str) -> PyScanResult`.
///
/// Walks the directory at `path` (non-recursive), discovers all
/// EMIR `.xml` and `.parquet` files (plus expands any `.zip` /
/// `.gz` archives the same way the CLI does), parses each into
/// `EmirRecord`s, aggregates the lot into one big in-memory
/// batch, and runs the standard `default_checks()` suite over
/// the aggregate. `summary.files_processed` reflects the actual
/// number of files contributed records.
///
/// **`.csv` files are rejected** in v0.13.0 (CSV requires a
/// column mapping ; see error message for the workaround).
#[pyfunction]
#[pyo3(signature = (path, *, normalize = false))]
pub fn scan_directory(path: &str, normalize: bool) -> PyResult<PyScanResult> {
    let paths = opendqi_io::discover_emir_inputs(Path::new(path)).map_err(to_py_err)?;
    scan_emir_paths(&paths, normalize)
}

/// `opendqi.emir.scan_files(paths: list[str]) -> PyScanResult`.
///
/// Explicit-list variant of `scan_directory` — skips the
/// directory walk and parses exactly the paths the caller
/// provides. Same extension contract (`.xml`, `.parquet`) and
/// same aggregation behaviour.
#[pyfunction]
#[pyo3(signature = (paths, *, normalize = false))]
pub fn scan_files(paths: Vec<String>, normalize: bool) -> PyResult<PyScanResult> {
    let path_bufs: Vec<std::path::PathBuf> = paths.iter().map(std::path::PathBuf::from).collect();
    scan_emir_paths(&path_bufs, normalize)
}

/// Shared helper for `scan_directory` + `scan_files`: parse each
/// path, aggregate the records, run the standard EMIR scan
/// pipeline. `files_processed` = number of paths actually
/// contributing records.
fn scan_emir_paths(paths: &[std::path::PathBuf], normalize: bool) -> PyResult<PyScanResult> {
    if paths.is_empty() {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "scan_directory / scan_files received no input files (empty \
             directory or empty list). Provide at least one .xml or .parquet path.",
        ));
    }
    let mut all_records: Vec<EmirRecord> = Vec::new();
    for p in paths {
        let recs = read_emir_path_as_records(p)?;
        all_records.extend(recs);
    }
    let files = paths.len() as u32;
    scan_emir_records(all_records, files, normalize)
}

// =================================================================
// v0.13.0 — cross-message workflow: tr_audit
// =================================================================

/// `opendqi.emir.tr_audit(*, tar, tsr, feedback) -> PyScanResult`.
///
/// Consolidates a 3-layer TR audit in a single call : parses the
/// TAR (`auth.030`), TSR (`auth.107`) and feedback (`auth.092`)
/// XML files, runs each layer's check pack on its records, then
/// runs the 3 `EMIR.AUD.*` cross-layer coherence checks
/// (`REJECTED_BUT_OUTSTANDING_IN_TSR`,
/// `NEWT_IN_TAR_NOT_IN_TSR`, `OUTSTANDING_IN_TSR_NOT_IN_TAR`).
///
/// All resulting issues are merged into a single `PyScanResult`
/// whose `summary.files_processed = 3` and whose `issues` Arrow
/// table contains the union (with `check_id` prefixes
/// distinguishing the layer : `EMIR.{COMP,VLD,ACC,...}` for TAR,
/// `EMIR.TST.*` for TSR, `EMIR.FBK.*` for feedback, and the 3
/// `EMIR.AUD.*` for the cross-layer audit).
///
/// **Store-backed lifecycle is NOT included** in v0.13.0 — for
/// `--store` cross-batch lifecycle checks use the CLI
/// (`opendqi emir tr-audit --store path.db ...`). Python wrapper
/// for the SQLite store is deferred to v0.14+.
#[pyfunction]
#[pyo3(signature = (*, tar, tsr, feedback))]
pub fn tr_audit(tar: &str, tsr: &str, feedback: &str) -> PyResult<PyScanResult> {
    let started_at = Utc::now();
    let ctx = CheckContext::now_with_defaults();
    let thresholds = Thresholds::default();

    // Parse the three layers (format-level issues from each
    // parser are dropped here — consistent with scan_directory /
    // scan_files. A future v0.14 may add `include_format_issues`).
    let tar_records = opendqi_xml::read_emir_xml(Path::new(tar))
        .map_err(to_py_err)?
        .records;
    let tsr_records = opendqi_xml::read_emir_tr_state_xml(Path::new(tsr))
        .map_err(to_py_err)?
        .records;
    let fbk_records = opendqi_xml::read_emir_feedback_xml(Path::new(feedback))
        .map_err(to_py_err)?
        .records;

    let sink = Mutex::new(SortedIssueSink::new(&thresholds, STREAM_SPILL_MAX_ISSUES));

    // Layer 1 — TAR records ⊗ default_checks (single-batch EMIR DQ).
    let tar_checks = default_checks();
    stream_emir_checks_into(&tar_checks, &tar_records, &ctx, &sink);

    // Layer 2 — TSR records ⊗ default_tr_state_checks. Uses the
    // generic `stream_checks_into` because TrStateCheck is a
    // distinct trait from the `Check` (EmirRecord) one. The
    // `prior` arg is the history-store-loaded EmirRecord set —
    // Python doesn't touch the store in v0.13, so we pass an
    // empty slice (lifecycle-flavored checks that need history
    // simply produce no issues).
    let tsr_checks = default_tr_state_checks();
    let prior_emir: &[EmirRecord] = &[];
    stream_checks_into(&tsr_checks, &sink, |c| {
        c.run(&tsr_records, prior_emir, &ctx)
    });

    // Layer 3 — feedback records ⊗ default_feedback_checks (also
    // takes empty prior; see TSR comment).
    let fbk_checks = default_feedback_checks();
    stream_checks_into(&fbk_checks, &sink, |c| {
        c.run(&fbk_records, prior_emir, &ctx)
    });

    // Cross-layer — the 3 EMIR.AUD.* checks that only this
    // command can run (each needs records from at least 2 of the
    // 3 layers).
    let cross =
        compute_tr_audit_emir_issues(&tar_records, &tsr_records, &fbk_records, tsr, feedback);
    if !cross.is_empty() {
        sink.lock()
            .expect("sink mutex not poisoned")
            .push_batch(cross);
    }

    let finished_at = Utc::now();
    let records_total = (tar_records.len() + tsr_records.len() + fbk_records.len()) as u32;
    let (summary, sorted_issues) = sink.into_inner().expect("sink mutex not poisoned").finish(
        Regime::Emir,
        3,
        records_total,
        started_at,
        finished_at,
    );
    let batch = issues_to_record_batch(sorted_issues).map_err(to_py_err)?;
    Ok(PyScanResult::new(summary, Some(batch), None))
}

// =================================================================
// v0.13.0 — cross-message workflow: collateral_audit
// =================================================================

/// `opendqi.emir.collateral_audit(*, tsr, msr) -> PyScanResult`.
///
/// EMIR Article 11 collateral obligation check — joins the TSR
/// (`auth.107`, trade state) with the MSR (`auth.109`, margin
/// state) by UTI and emits the 2 `EMIR.COL.*` cross-message
/// checks (`EMIR.COL.MISSING` when no MSR link exists or all 4
/// IM/VM posted+collected amounts are absent/zero ;
/// `EMIR.COL.STALE` when the linked margin snapshot is older
/// than `emir_rmt.collateral_max_age_days` vs the TSR
/// `state_as_of`). Mirror of the v0.10.0 CLI
/// `opendqi emir collateral-audit --tsr ... --msr ...`.
///
/// Default thresholds (`Thresholds::default()`) — Python can't
/// override the threshold config in v0.13.0. Users who need
/// custom thresholds (`collateral_max_age_days`, severity
/// overrides, ...) go through the CLI with `--config
/// thresholds.yml`. Programmatic threshold passthrough is a
/// v0.14+ item.
#[pyfunction]
#[pyo3(signature = (*, tsr, msr))]
pub fn collateral_audit(tsr: &str, msr: &str) -> PyResult<PyScanResult> {
    let started_at = Utc::now();
    let thresholds = Thresholds::default();

    let tsr_records = opendqi_xml::read_emir_tr_state_xml(Path::new(tsr))
        .map_err(to_py_err)?
        .records;
    let msr_records = opendqi_xml::read_emir_msr_xml(Path::new(msr))
        .map_err(to_py_err)?
        .records;

    let now = Utc::now();
    let cross = compute_collateral_emir_issues(&tsr_records, &msr_records, &thresholds, now);

    let sink = Mutex::new(SortedIssueSink::new(&thresholds, STREAM_SPILL_MAX_ISSUES));
    if !cross.is_empty() {
        sink.lock()
            .expect("sink mutex not poisoned")
            .push_batch(cross);
    }

    let finished_at = Utc::now();
    let records_total = (tsr_records.len() + msr_records.len()) as u32;
    let (summary, sorted_issues) = sink.into_inner().expect("sink mutex not poisoned").finish(
        Regime::Emir,
        2,
        records_total,
        started_at,
        finished_at,
    );
    let batch = issues_to_record_batch(sorted_issues).map_err(to_py_err)?;
    Ok(PyScanResult::new(summary, Some(batch), None))
}

// =================================================================
// v0.13.0 — cross-message workflow: book_reconcile
// =================================================================

/// `opendqi.emir.book_reconcile(book, tsr, *, mapping=None) -> PyScanResult`.
///
/// Reconciles a firm's internal book against the TR Trade State
/// Report. `book` can be either:
///   - a `str` path to a `.csv` (needs `mapping`) or `.parquet`
///     (no mapping needed — parquet schema is canonical)
///   - a `pyarrow.Table` / `pyarrow.RecordBatch` already in
///     memory (needs `mapping`)
///
/// `tsr` is always a path to the auth.107 XML.
///
/// Fires the 5 `EMIR.BREC.*` checks (NOTIONAL_MISMATCH,
/// NOTIONAL_CURRENCY_MISMATCH, VALUATION_MISMATCH,
/// MATURITY_MISMATCH, STATUS_MISMATCH).
///
/// The `mapping` dict direction is `canonical_field_name →
/// user_column_name` (same as `scan_table` and the existing
/// CSV mapping YAML format).
///
/// v0.14.0: `date_format` and `datetime_format` kwargs let
/// callers override the CSV parser's date/datetime parsing
/// formats. Defaults are `%Y-%m-%d` (date) and RFC 3339
/// (datetime) — the same as `CsvMapping::effective_*_format`.
/// Ignored when `book` is a pyarrow.Table/RecordBatch (the
/// Arrow input path doesn't go through `CsvMapping`).
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

    // Parse the TSR side (always XML).
    let tsr_records = opendqi_xml::read_emir_tr_state_xml(Path::new(tsr))
        .map_err(to_py_err)?
        .records;

    // Dispatch the `book` input: str → file path (CSV or Parquet);
    // anything else → assume a pyarrow Table / RecordBatch.
    let book_records: Vec<EmirRecord> = if let Ok(path_str) = book.extract::<String>() {
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
                        "book_reconcile with a .csv path requires `mapping={...}` \
                         (a dict of canonical_field → csv_column_name).",
                    )
                })?;
                let csv_mapping = CsvMapping {
                    fields: mapping.into_iter().collect(), // HashMap → BTreeMap
                    date_format: date_format.clone(),
                    datetime_format: datetime_format.clone(),
                };
                opendqi_io::read_emir_csv(path, &csv_mapping).map_err(to_py_err)?
            }
            "parquet" => {
                // Parquet path uses the canonical schema directly;
                // `mapping` is ignored (no remapping needed).
                opendqi_io::read_emir_parquet(path).map_err(to_py_err)?
            }
            other => {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "book_reconcile: unsupported file extension {:?} for book path \
                     (expected .csv or .parquet). Offending path: {}",
                    other,
                    path.display()
                )));
            }
        }
    } else {
        // pyarrow.Table / RecordBatch path — needs a mapping.
        let mapping = mapping.ok_or_else(|| {
            pyo3::exceptions::PyValueError::new_err(
                "book_reconcile with a pyarrow.Table/RecordBatch requires \
                 `mapping={...}` (a dict of canonical_field → arrow_column_name).",
            )
        })?;
        let batch = pyarrow_to_record_batch(book)?;
        crate::convert::batch_to_emir_records(&batch, &mapping as &Mapping).map_err(to_py_err)?
    };

    // Run the 5 EMIR.BREC.* checks.
    let cross = compute_book_reconcile_issues(&book_records, &tsr_records);

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
        Regime::Emir,
        2,
        records_total,
        started_at,
        finished_at,
    );
    let batch = issues_to_record_batch(sorted_issues).map_err(to_py_err)?;
    Ok(PyScanResult::new(summary, Some(batch), None))
}

// =================================================================
// v0.15 — EMIR Data Quality Pack
// =================================================================

/// `opendqi.emir.data_quality_pack(*, tsr=None, tar=None, msr=None,
/// mar=None, feedback=None, mappings=None, as_of=None) ->
/// PyDqiPackResult`.
///
/// EMIR Data Quality Pack (v0.15): aggregates the 5 EMIR TR layers
/// into 10 regulator-style indicators (numerator / denominator /
/// rate / threshold / status) + ≤ 20 evidence rows per indicator,
/// AND co-produces the granular issue stream from the existing
/// 216-check registries. All inputs are optional ; at least one
/// must be provided.
///
/// **Dual input** (v0.15.0): each of `tsr` / `tar` / `msr` /
/// `feedback` accepts either:
/// - a `str` file path (XML), OR
/// - a `pyarrow.Table` (or `pyarrow.RecordBatch`) — requires a
///   matching entry in `mappings` (per-layer dict
///   `{canonical_field: arrow_column_name}`).
///
/// `mar` is **paths-only** in v0.15 (no Arrow input). Passing a
/// pyarrow.Table for `mar` raises ValueError. Granular MAR
/// checks still run when an XML path is provided. MAR Arrow
/// input = follow-up.
///
/// Custom thresholds via programmatic YAML config are deferred —
/// users needing overrides use the CLI's `--config`.
///
/// `as_of` defaults to today (UTC). Format: `YYYY-MM-DD`.
/// Pinning it keeps the stale-valuation cutoff stable across
/// calendar days (the CLI golden uses this same trick).
///
/// Gating: the orchestrator inspects the loaded TAR records'
/// `raw_fields` for `confirmation_timestamp` /
/// `reconciliation_status` ; if either is observed at least
/// once non-empty, the matching `DQI_CONF_MISSING` /
/// `DQI_REC_STATUS_UNPAIRED` indicator computes. Otherwise it
/// reports `status: not_applicable` with no rate.
#[pyfunction]
#[pyo3(signature = (*, tsr=None, tar=None, msr=None, mar=None, feedback=None, positions=None, recon_stats=None, mappings=None, as_of=None))]
#[allow(clippy::too_many_arguments)]
pub fn data_quality_pack<'py>(
    tsr: Option<&Bound<'py, PyAny>>,
    tar: Option<&Bound<'py, PyAny>>,
    msr: Option<&Bound<'py, PyAny>>,
    mar: Option<&Bound<'py, PyAny>>,
    feedback: Option<&Bound<'py, PyAny>>,
    positions: Option<&str>,
    recon_stats: Option<&str>,
    mappings: Option<HashMap<String, HashMap<String, String>>>,
    as_of: Option<&str>,
) -> PyResult<PyDqiPackResult> {
    if tsr.is_none()
        && tar.is_none()
        && msr.is_none()
        && mar.is_none()
        && feedback.is_none()
        && positions.is_none()
        && recon_stats.is_none()
    {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "data_quality_pack: at least one of tsr / tar / msr / mar / feedback / \
             positions / recon_stats is required",
        ));
    }

    let thresholds = Thresholds::default();

    // Resolve as_of (default = today UTC).
    let as_of_date = match as_of {
        Some(s) => chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|e| {
            pyo3::exceptions::PyValueError::new_err(format!(
                "parsing as_of {s:?} (expected YYYY-MM-DD): {e}"
            ))
        })?,
        None => Utc::now().date_naive(),
    };

    // MAR is paths-only in v0.15 — reject Arrow input early with a
    // clear message rather than letting it silently degrade.
    if let Some(obj) = mar {
        if obj.extract::<String>().is_err() {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "data_quality_pack: `mar` accepts a str path only in v0.15 \
                 (pyarrow.Table for MAR will be added in v0.16). Pass an \
                 XML path or omit the argument.",
            ));
        }
    }

    let layer_mapping =
        |layer: &str| -> Option<&HashMap<String, String>> { mappings.as_ref()?.get(layer) };

    // Load each provided layer (dual dispatch: str path vs pyarrow.Table).
    let tsr_records = load_tsr(tsr, layer_mapping("tsr"))?;
    let tar_records = load_tar(tar, layer_mapping("tar"))?;
    let msr_records = load_msr(msr, layer_mapping("msr"))?;
    let mar_records = match mar {
        Some(obj) => {
            let p: String = obj.extract().expect("validated above");
            opendqi_xml::read_emir_mar_xml(Path::new(&p))
                .map_err(to_py_err)?
                .records
        }
        None => Vec::new(),
    };
    let feedback_records = load_feedback(feedback, layer_mapping("feedback"))?;
    // v0.18 E7: positions = str path only (Arrow input deferred
    // beyond v0.18 scope).
    let position_records = match positions {
        Some(p) => {
            opendqi_xml::read_emir_position_set_xml(Path::new(p))
                .map_err(to_py_err)?
                .records
        }
        None => Vec::new(),
    };
    // v0.18 F1: auth.091 dual-purpose parse (str path only;
    // populates BOTH recon_stats + reconciliation slots).
    let (recon_stats_records, reconciliation_records) = match recon_stats {
        Some(p) => {
            let outcome = opendqi_xml::read_emir_recon_stats_xml(Path::new(p))
                .map_err(to_py_err)?;
            (outcome.records, outcome.reconciliation_records)
        }
        None => (Vec::new(), Vec::new()),
    };

    // Detect raw_fields presence for the 2 gated indicators
    // (same heuristic as the CLI run_data_quality_pack).
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

    let inputs = EmirDqiInputs {
        tsr: tsr.map(|_| tsr_records.as_slice()),
        tar: tar.map(|_| tar_records.as_slice()),
        msr: msr.map(|_| msr_records.as_slice()),
        mar: mar.map(|_| mar_records.as_slice()),
        feedback: feedback.map(|_| feedback_records.as_slice()),
        // v0.16 B1 — recon_stats + reconciliation are not yet
        // exposed in the Python signature ; the new cross-CP
        // DQIs self-report NotApplicable until a follow-up
        // commit threads --recon-stats / --reconciliation
        // through both the CLI and the Python binding.
        // v0.18 F1 — recon_stats + reconciliation BOTH wired
        // from the single `recon_stats=` kwarg (one auth.091
        // file -> two slot vectors).
        recon_stats: recon_stats.map(|_| recon_stats_records.as_slice()),
        reconciliation: recon_stats.map(|_| reconciliation_records.as_slice()),
        // v0.18 E7 — positions slot wired through `positions=` kwarg.
        positions: positions.map(|_| position_records.as_slice()),
    };

    let pack = compute_emir_dqi_pack(inputs, mapping_presence, &thresholds, as_of_date);

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
// Dual-input helpers for `data_quality_pack` — str path or pyarrow.Table
// =================================================================

/// Common error for "Arrow input but no mapping for this layer".
fn missing_mapping_err(layer: &str) -> PyErr {
    pyo3::exceptions::PyValueError::new_err(format!(
        "data_quality_pack: a pyarrow.Table was passed for `{layer}` but \
         `mappings={{...}}` does not contain an entry for `{layer}`. \
         Provide e.g. mappings={{\"{layer}\": {{\"uti\": \"YourUtiCol\", ...}}}}"
    ))
}

fn load_tsr<'py>(
    obj: Option<&Bound<'py, PyAny>>,
    mapping: Option<&HashMap<String, String>>,
) -> PyResult<Vec<opendqi_core::TrStateRecord>> {
    match obj {
        None => Ok(Vec::new()),
        Some(o) => {
            if let Ok(path) = o.extract::<String>() {
                Ok(opendqi_xml::read_emir_tr_state_xml(Path::new(&path))
                    .map_err(to_py_err)?
                    .records)
            } else {
                let m = mapping.ok_or_else(|| missing_mapping_err("tsr"))?;
                let batch = pyarrow_to_record_batch(o)?;
                crate::convert::batch_to_tr_state_records(&batch, m).map_err(to_py_err)
            }
        }
    }
}

fn load_tar<'py>(
    obj: Option<&Bound<'py, PyAny>>,
    mapping: Option<&HashMap<String, String>>,
) -> PyResult<Vec<EmirRecord>> {
    match obj {
        None => Ok(Vec::new()),
        Some(o) => {
            if let Ok(path) = o.extract::<String>() {
                Ok(opendqi_xml::read_emir_xml(Path::new(&path))
                    .map_err(to_py_err)?
                    .records)
            } else {
                let m = mapping.ok_or_else(|| missing_mapping_err("tar"))?;
                let batch = pyarrow_to_record_batch(o)?;
                batch_to_emir_records(&batch, m).map_err(to_py_err)
            }
        }
    }
}

fn load_msr<'py>(
    obj: Option<&Bound<'py, PyAny>>,
    mapping: Option<&HashMap<String, String>>,
) -> PyResult<Vec<opendqi_core::MarginStateRecord>> {
    match obj {
        None => Ok(Vec::new()),
        Some(o) => {
            if let Ok(path) = o.extract::<String>() {
                Ok(opendqi_xml::read_emir_msr_xml(Path::new(&path))
                    .map_err(to_py_err)?
                    .records)
            } else {
                let m = mapping.ok_or_else(|| missing_mapping_err("msr"))?;
                let batch = pyarrow_to_record_batch(o)?;
                crate::convert::batch_to_margin_state_records(&batch, m).map_err(to_py_err)
            }
        }
    }
}

fn load_feedback<'py>(
    obj: Option<&Bound<'py, PyAny>>,
    mapping: Option<&HashMap<String, String>>,
) -> PyResult<Vec<opendqi_core::FeedbackRecord>> {
    match obj {
        None => Ok(Vec::new()),
        Some(o) => {
            if let Ok(path) = o.extract::<String>() {
                Ok(opendqi_xml::read_emir_feedback_xml(Path::new(&path))
                    .map_err(to_py_err)?
                    .records)
            } else {
                let m = mapping.ok_or_else(|| missing_mapping_err("feedback"))?;
                let batch = pyarrow_to_record_batch(o)?;
                crate::convert::batch_to_feedback_records(&batch, m).map_err(to_py_err)
            }
        }
    }
}

// =================================================================
// v0.20 Phase C — per-layer Python wrappers (parity with CLI per-
// layer subcommands). Each wrapper parses the layer XML, runs the
// matching default_*_checks suite against the records, and returns
// a PyScanResult identical in shape to the v0.12+ surface.
// =================================================================

/// `opendqi.emir.mar_scan(path: str) -> PyScanResult`. ISO 20022
/// auth.108 EMIR Margin Activity Report. Mirror of `opendqi emir
/// mar-scan` (no store / cross-batch lifecycle from Python in v0.20;
/// the CLI handles the SQLite store path).
#[pyfunction]
pub fn mar_scan(path: &str) -> PyResult<PyScanResult> {
    let started_at = Utc::now();
    let outcome = opendqi_xml::read_emir_mar_xml(Path::new(path)).map_err(to_py_err)?;
    let ctx = CheckContext::now_with_defaults();
    let thresholds = Thresholds::default();
    let sink = Mutex::new(SortedIssueSink::new(&thresholds, STREAM_SPILL_MAX_ISSUES));
    sink.lock().expect("sink").push_batch(outcome.issues);
    let prior: Vec<opendqi_core::MarginActivityRecord> = Vec::new();
    stream_checks_into(&default_margin_activity_checks(), &sink, |c| {
        c.run(&outcome.records, &prior, &ctx)
    });
    let n = outcome.records.len() as u32;
    let (summary, sorted) = sink
        .into_inner()
        .expect("sink mutex")
        .finish(Regime::Emir, 1, n, started_at, Utc::now());
    let batch = issues_to_record_batch(sorted).map_err(to_py_err)?;
    Ok(PyScanResult::new(summary, Some(batch), None))
}

/// `opendqi.emir.msr_scan(path: str) -> PyScanResult`. ISO 20022
/// auth.109 EMIR Margin State Report. Mirror of `opendqi emir
/// msr-scan`.
#[pyfunction]
pub fn msr_scan(path: &str) -> PyResult<PyScanResult> {
    let started_at = Utc::now();
    let outcome = opendqi_xml::read_emir_msr_xml(Path::new(path)).map_err(to_py_err)?;
    let ctx = CheckContext::now_with_defaults();
    let thresholds = Thresholds::default();
    let sink = Mutex::new(SortedIssueSink::new(&thresholds, STREAM_SPILL_MAX_ISSUES));
    sink.lock().expect("sink").push_batch(outcome.issues);
    let prior: Vec<EmirRecord> = Vec::new();
    stream_checks_into(&default_margin_state_checks(), &sink, |c| {
        c.run(&outcome.records, &prior, &ctx)
    });
    let n = outcome.records.len() as u32;
    let (summary, sorted) = sink
        .into_inner()
        .expect("sink mutex")
        .finish(Regime::Emir, 1, n, started_at, Utc::now());
    let batch = issues_to_record_batch(sorted).map_err(to_py_err)?;
    Ok(PyScanResult::new(summary, Some(batch), None))
}

/// `opendqi.emir.recon_stats(path: str, *, prior: str | None = None)
/// -> PyScanResult`. ISO 20022 auth.091 EMIR Reconciliation
/// Statistical Report. Optional `prior=` kwarg activates the
/// batch-over-batch pairing-rate-trend check.
#[pyfunction]
#[pyo3(signature = (path, *, prior = None))]
pub fn recon_stats(path: &str, prior: Option<&str>) -> PyResult<PyScanResult> {
    let started_at = Utc::now();
    let outcome = opendqi_xml::read_emir_recon_stats_xml(Path::new(path)).map_err(to_py_err)?;
    let prior_records = match prior {
        Some(p) => {
            opendqi_xml::read_emir_recon_stats_xml(Path::new(p))
                .map_err(to_py_err)?
                .records
        }
        None => Vec::new(),
    };
    let ctx = CheckContext::now_with_defaults();
    let thresholds = Thresholds::default();
    let sink = Mutex::new(SortedIssueSink::new(&thresholds, STREAM_SPILL_MAX_ISSUES));
    sink.lock().expect("sink").push_batch(outcome.issues);
    stream_checks_into(&default_recon_stats_checks(), &sink, |c| {
        c.run(&outcome.records, &prior_records, &ctx)
    });
    let n = outcome.records.len() as u32;
    let (summary, sorted) = sink
        .into_inner()
        .expect("sink mutex")
        .finish(Regime::Emir, 1, n, started_at, Utc::now());
    let batch = issues_to_record_batch(sorted).map_err(to_py_err)?;
    Ok(PyScanResult::new(summary, Some(batch), None))
}

/// `opendqi.emir.warnings(path: str) -> PyScanResult`. ISO 20022
/// auth.106 EMIR Trade Warnings Report. Mirror of `opendqi emir
/// warnings`.
#[pyfunction]
pub fn warnings(path: &str) -> PyResult<PyScanResult> {
    let started_at = Utc::now();
    let outcome = opendqi_xml::read_emir_warnings_xml(Path::new(path)).map_err(to_py_err)?;
    let ctx = CheckContext::now_with_defaults();
    let thresholds = Thresholds::default();
    let sink = Mutex::new(SortedIssueSink::new(&thresholds, STREAM_SPILL_MAX_ISSUES));
    sink.lock().expect("sink").push_batch(outcome.issues);
    stream_checks_into(&default_warnings_checks(), &sink, |c| {
        c.run(&outcome.records, &ctx)
    });
    let n = outcome.records.len() as u32;
    let (summary, sorted) = sink
        .into_inner()
        .expect("sink mutex")
        .finish(Regime::Emir, 1, n, started_at, Utc::now());
    let batch = issues_to_record_batch(sorted).map_err(to_py_err)?;
    Ok(PyScanResult::new(summary, Some(batch), None))
}

/// `opendqi.emir.feedback(path: str) -> PyScanResult`. ISO 20022
/// auth.092 EMIR Trade Rejection Statistical Report. Python equivalent
/// of `opendqi emir feedback` without the SQLite store integration
/// (Python store wrapper deferred to v0.21+; for store-backed
/// lifecycle use the CLI).
#[pyfunction]
pub fn feedback(path: &str) -> PyResult<PyScanResult> {
    let started_at = Utc::now();
    let outcome = opendqi_xml::read_emir_feedback_xml(Path::new(path)).map_err(to_py_err)?;
    let ctx = CheckContext::now_with_defaults();
    let thresholds = Thresholds::default();
    let sink = Mutex::new(SortedIssueSink::new(&thresholds, STREAM_SPILL_MAX_ISSUES));
    sink.lock().expect("sink").push_batch(outcome.issues);
    let prior: Vec<EmirRecord> = Vec::new();
    stream_checks_into(&default_feedback_checks(), &sink, |c| {
        c.run(&outcome.records, &prior, &ctx)
    });
    let n = outcome.records.len() as u32;
    let (summary, sorted) = sink
        .into_inner()
        .expect("sink mutex")
        .finish(Regime::Emir, 1, n, started_at, Utc::now());
    let batch = issues_to_record_batch(sorted).map_err(to_py_err)?;
    Ok(PyScanResult::new(summary, Some(batch), None))
}

/// `opendqi.emir.query_scan(path: str) -> PyScanResult`. ISO 20022
/// auth.029 EMIR Trade Report Query (firm-side query envelope).
/// Envelope-only — fires only EMIR.QRY.ENVELOPE_WELLFORMED. v0.20.
#[pyfunction]
pub fn query_scan(path: &str) -> PyResult<PyScanResult> {
    let started_at = Utc::now();
    let outcome = opendqi_xml::read_emir_query_xml(Path::new(path)).map_err(to_py_err)?;
    let ctx = CheckContext::now_with_defaults();
    let thresholds = Thresholds::default();
    let sink = Mutex::new(SortedIssueSink::new(&thresholds, STREAM_SPILL_MAX_ISSUES));
    sink.lock().expect("sink").push_batch(outcome.issues);
    stream_checks_into(&default_emir_query_checks(), &sink, |c| {
        c.run(&outcome.records, &ctx)
    });
    let n = outcome.records.len() as u32;
    let (summary, sorted) = sink
        .into_inner()
        .expect("sink mutex")
        .finish(Regime::Emir, 1, n, started_at, Utc::now());
    let batch = issues_to_record_batch(sorted).map_err(to_py_err)?;
    Ok(PyScanResult::new(summary, Some(batch), None))
}

/// `opendqi.emir.status_advice_scan(path: str) -> PyScanResult`.
/// ISO 20022 auth.031 EMIR Status Advice (TR → firm ack envelope).
/// Envelope-only — fires only EMIR.ACK.ENVELOPE_WELLFORMED. v0.20.
#[pyfunction]
pub fn status_advice_scan(path: &str) -> PyResult<PyScanResult> {
    let started_at = Utc::now();
    let outcome =
        opendqi_xml::read_emir_status_advice_xml(Path::new(path)).map_err(to_py_err)?;
    let ctx = CheckContext::now_with_defaults();
    let thresholds = Thresholds::default();
    let sink = Mutex::new(SortedIssueSink::new(&thresholds, STREAM_SPILL_MAX_ISSUES));
    sink.lock().expect("sink").push_batch(outcome.issues);
    stream_checks_into(&default_emir_status_advice_checks(), &sink, |c| {
        c.run(&outcome.records, &ctx)
    });
    let n = outcome.records.len() as u32;
    let (summary, sorted) = sink
        .into_inner()
        .expect("sink mutex")
        .finish(Regime::Emir, 1, n, started_at, Utc::now());
    let batch = issues_to_record_batch(sorted).map_err(to_py_err)?;
    Ok(PyScanResult::new(summary, Some(batch), None))
}

// =================================================================
// Module registration
// =================================================================

/// Register the `opendqi.emir` submodule on the parent
/// `opendqi` module.
pub fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = parent.py();
    let m = PyModule::new_bound(py, "emir")?;
    m.add("__doc__", "EMIR data-quality bindings.")?;
    // Single-file entry points (v0.12.0).
    m.add_function(wrap_pyfunction!(scan_parquet, &m)?)?;
    m.add_function(wrap_pyfunction!(scan_table, &m)?)?;
    m.add_function(wrap_pyfunction!(parse_xml, &m)?)?;
    // Multi-file entry points (v0.13.0).
    m.add_function(wrap_pyfunction!(scan_directory, &m)?)?;
    m.add_function(wrap_pyfunction!(scan_files, &m)?)?;
    // Cross-message workflow entry points (v0.13.0).
    m.add_function(wrap_pyfunction!(tr_audit, &m)?)?;
    m.add_function(wrap_pyfunction!(collateral_audit, &m)?)?;
    m.add_function(wrap_pyfunction!(book_reconcile, &m)?)?;
    // Data Quality Pack (v0.15.0).
    m.add_function(wrap_pyfunction!(data_quality_pack, &m)?)?;
    // Per-layer scan wrappers (v0.20 Phase C — parity with CLI).
    m.add_function(wrap_pyfunction!(mar_scan, &m)?)?;
    m.add_function(wrap_pyfunction!(msr_scan, &m)?)?;
    m.add_function(wrap_pyfunction!(recon_stats, &m)?)?;
    m.add_function(wrap_pyfunction!(warnings, &m)?)?;
    m.add_function(wrap_pyfunction!(feedback, &m)?)?;
    m.add_function(wrap_pyfunction!(query_scan, &m)?)?;
    m.add_function(wrap_pyfunction!(status_advice_scan, &m)?)?;
    parent.add_submodule(&m)?;
    Ok(())
}

// =================================================================
// PyArrow helpers (shared with sftr.rs via crate-private re-export)
// =================================================================

/// Convert a pyarrow `Table` (multi-chunk) or `RecordBatch` (single)
/// into a single `arrow::array::RecordBatch`. The standard
/// pyarrow Table → single batch coalescence path is
/// `.combine_chunks().to_batches()[0]`.
pub(crate) fn pyarrow_to_record_batch<'py>(obj: &Bound<'py, PyAny>) -> PyResult<RecordBatch> {
    // If it has `combine_chunks`, it's a pyarrow.Table — combine and
    // take the first (and only) batch. Otherwise assume it's already
    // a RecordBatch and let `FromPyArrow` parse it.
    let combined: Bound<'py, PyAny> = if obj.hasattr("combine_chunks")? {
        let t = obj.call_method0("combine_chunks")?;
        let batches = t.call_method0("to_batches")?;
        let batches_list = batches.downcast_into::<PyList>()?;
        if batches_list.is_empty() {
            // Empty pyarrow.Table — produce a zero-row RecordBatch
            // with the input table's schema by going through pyarrow
            // again. Easier: just take the schema and build an
            // empty batch on the Rust side.
            // Edge case kept simple — most users have at least 1 row.
            return Err(pyo3::exceptions::PyValueError::new_err(
                "scan_table requires a non-empty pyarrow.Table",
            ));
        }
        batches_list.get_item(0)?
    } else {
        // pyarrow.RecordBatch — usable directly via FromPyArrow.
        obj.clone()
    };
    RecordBatch::from_pyarrow_bound(&combined).map_err(|e| {
        pyo3::exceptions::PyTypeError::new_err(format!(
            "could not convert input to an Arrow RecordBatch: {e}"
        ))
    })
}

/// Wrap an arrow `RecordBatch` in a single-chunk `pyarrow.Table`.
/// Same boilerplate as `PyScanResult::issues`'s getter — kept
/// here for `parse_xml`.
pub(crate) fn record_batch_to_pyarrow_table<'py>(
    py: Python<'py>,
    batch: &RecordBatch,
) -> PyResult<Bound<'py, PyAny>> {
    let rb_py = batch.to_pyarrow(py)?;
    let pa = py.import_bound("pyarrow")?;
    let batches = PyList::new_bound(py, [rb_py]);
    let table = pa
        .getattr("Table")?
        .call_method1("from_batches", (batches,))?;
    Ok(table)
}
