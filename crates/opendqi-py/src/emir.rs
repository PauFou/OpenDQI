//! `opendqi.emir` submodule — EMIR-side bindings.
//!
//! Three entry points :
//! - `scan_parquet(path)`        — P2: scan a normalized EMIR
//!                                  Parquet, return summary + issues.
//! - `scan_table(table, mapping)` — P4: scan an Arrow table directly
//!                                  with a canonical-field → user-col
//!                                  mapping. Avoids the Parquet
//!                                  roundtrip.
//! - `parse_xml(path)`           — P4: parse any EMIR ISO 20022
//!                                  firm submission (auth.030,
//!                                  auth.052 SFTR-side) into an
//!                                  Arrow Table that can be passed
//!                                  back into `scan_table` or
//!                                  inspected directly.

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
/// and any future `scan_*` adder.
fn scan_emir_records(records: Vec<EmirRecord>, files: u32) -> PyResult<PyScanResult> {
    let started_at = Utc::now();
    let ctx = CheckContext::now_with_defaults();
    let checks = default_checks();
    let thresholds = Thresholds::default();
    let sink = Mutex::new(SortedIssueSink::new(&thresholds, STREAM_SPILL_MAX_ISSUES));
    stream_emir_checks_into(&checks, &records, &ctx, &sink);
    let finished_at = Utc::now();
    let n = records.len() as u32;
    let (summary, sorted_issues) = sink
        .into_inner()
        .expect("sink mutex not poisoned")
        .finish(Regime::Emir, files, n, started_at, finished_at);
    let batch = issues_to_record_batch(sorted_issues).map_err(to_py_err)?;
    Ok(PyScanResult::new(summary, Some(batch)))
}

/// `opendqi.emir.scan_parquet(path: str) -> PyScanResult`.
///
/// Reads the canonical EMIR Parquet at `path` (same schema as
/// `opendqi emir normalize --out`), runs the 89 single-batch EMIR
/// data-quality checks (`default_checks()`), and returns a
/// `PyScanResult` matching the on-disk `summary.json` shape +
/// the v1.0 stable Arrow schema for issues.
#[pyfunction]
pub fn scan_parquet(path: &str) -> PyResult<PyScanResult> {
    let records = opendqi_io::read_emir_parquet(Path::new(path)).map_err(to_py_err)?;
    scan_emir_records(records, 1)
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
#[pyo3(signature = (table, mapping))]
pub fn scan_table<'py>(
    table: &Bound<'py, PyAny>,
    mapping: HashMap<String, String>,
) -> PyResult<PyScanResult> {
    let batch = pyarrow_to_record_batch(table)?;
    let records = batch_to_emir_records(&batch, &mapping as &Mapping).map_err(to_py_err)?;
    scan_emir_records(records, 1)
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

/// Register the `opendqi.emir` submodule on the parent
/// `opendqi` module.
pub fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = parent.py();
    let m = PyModule::new_bound(py, "emir")?;
    m.add("__doc__", "EMIR data-quality bindings.")?;
    m.add_function(wrap_pyfunction!(scan_parquet, &m)?)?;
    m.add_function(wrap_pyfunction!(scan_table, &m)?)?;
    m.add_function(wrap_pyfunction!(parse_xml, &m)?)?;
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
