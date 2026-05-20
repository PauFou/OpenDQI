//! `opendqi.sftr` submodule — SFTR-side bindings (symmetric to `emir`).

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use chrono::Utc;
use pyo3::prelude::*;
use pyo3::types::PyAny;

use opendqi_core::dq::{default_sftr_checks, stream_checks_into, CheckContext};
use opendqi_core::{Regime, SftrRecord, SortedIssueSink, Thresholds};

use crate::convert::{batch_to_sftr_records, Mapping};
use crate::emir::{pyarrow_to_record_batch, record_batch_to_pyarrow_table};
use crate::errors::to_py_err;
use crate::issues::issues_to_record_batch;
use crate::result::PyScanResult;

const STREAM_SPILL_MAX_ISSUES: usize = 65_536;

fn scan_sftr_records(records: Vec<SftrRecord>, files: u32) -> PyResult<PyScanResult> {
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
    let (summary, sorted_issues) = sink
        .into_inner()
        .expect("sink mutex not poisoned")
        .finish(Regime::Sftr, files, n, started_at, finished_at);
    let batch = issues_to_record_batch(sorted_issues).map_err(to_py_err)?;
    Ok(PyScanResult::new(summary, Some(batch)))
}

/// `opendqi.sftr.scan_parquet(path: str) -> PyScanResult`.
#[pyfunction]
pub fn scan_parquet(path: &str) -> PyResult<PyScanResult> {
    let records = opendqi_io::read_sftr_parquet(Path::new(path)).map_err(to_py_err)?;
    scan_sftr_records(records, 1)
}

/// `opendqi.sftr.scan_table(table, mapping={...}) -> PyScanResult`.
/// Same contract as `opendqi.emir.scan_table` (see that function's
/// docstring for the mapping direction and type strictness).
#[pyfunction]
#[pyo3(signature = (table, mapping))]
pub fn scan_table<'py>(
    table: &Bound<'py, PyAny>,
    mapping: HashMap<String, String>,
) -> PyResult<PyScanResult> {
    let batch = pyarrow_to_record_batch(table)?;
    let records = batch_to_sftr_records(&batch, &mapping as &Mapping).map_err(to_py_err)?;
    scan_sftr_records(records, 1)
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

/// Register the `opendqi.sftr` submodule on the parent
/// `opendqi` module.
pub fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = parent.py();
    let m = PyModule::new_bound(py, "sftr")?;
    m.add("__doc__", "SFTR data-quality bindings.")?;
    m.add_function(wrap_pyfunction!(scan_parquet, &m)?)?;
    m.add_function(wrap_pyfunction!(scan_table, &m)?)?;
    m.add_function(wrap_pyfunction!(parse_xml, &m)?)?;
    parent.add_submodule(&m)?;
    Ok(())
}
