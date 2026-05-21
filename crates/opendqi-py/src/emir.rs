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

use opendqi_core::dq::{default_checks, stream_emir_checks_into, CheckContext};
use opendqi_core::{EmirRecord, Regime, SortedIssueSink, Thresholds};

use crate::convert::{batch_to_emir_records, Mapping};
use crate::errors::to_py_err;
use crate::issues::issues_to_record_batch;
use crate::result::PyScanResult;

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
fn scan_emir_records(records: Vec<EmirRecord>, files: u32, normalize: bool) -> PyResult<PyScanResult> {
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

    let (summary, sorted_issues) = sink
        .into_inner()
        .expect("sink mutex not poisoned")
        .finish(Regime::Emir, files, n, started_at, finished_at);
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
    let table = pa.getattr("Table")?.call_method1("from_batches", (batches,))?;
    Ok(table)
}
