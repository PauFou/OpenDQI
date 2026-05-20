//! `PyScanResult` — the Python-facing result object returned by
//! every `opendqi.{emir,sftr}.scan_*` function.
//!
//! P3: `issues` is now a real Arrow `pyarrow.Table` (the v1.0
//! contract). `normalized` is still `None` until P5.

use arrow::array::RecordBatch;
use arrow::pyarrow::ToPyArrow;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyList};

use opendqi_core::ScanSummary;

/// Result of an OpenDQI scan, exposed to Python.
///
/// Fields populated incrementally:
/// - `summary`    — always present (M0.21 IssueAggregator contract).
/// - `issues`     — `pyarrow.Table` with the v1.0 stable 11-column
///                  schema (see `crate::issues::issues_schema`).
///                  Always present on the standard `scan_parquet` /
///                  `scan_table` paths.
/// - `normalized` — `pyarrow.Table` mirroring the canonical record
///                  model (`opendqi_io::emir_schema()` /
///                  `sftr_schema()`). Present only when the caller
///                  passed `normalize=True` (default `False`);
///                  `None` otherwise.
#[pyclass(module = "opendqi", frozen)]
pub struct PyScanResult {
    pub(crate) summary: ScanSummary,
    pub(crate) issues_batch: Option<RecordBatch>,
    pub(crate) normalized_batch: Option<RecordBatch>,
}

impl PyScanResult {
    pub(crate) fn new(
        summary: ScanSummary,
        issues_batch: Option<RecordBatch>,
        normalized_batch: Option<RecordBatch>,
    ) -> Self {
        Self {
            summary,
            issues_batch,
            normalized_batch,
        }
    }
}

#[pymethods]
impl PyScanResult {
    /// The scan summary as a Python dict — same shape as the
    /// on-disk `summary.json` (M0.21 `IssueAggregator` contract).
    #[getter]
    fn summary<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        summary_to_dict(py, &self.summary)
    }

    /// Issues as a `pyarrow.Table` matching the v1.0 stable schema
    /// in `opendqi-py::issues::issues_schema`. Returns `None` only
    /// when the scan produced no `RecordBatch` (always present in
    /// the standard `scan_parquet` / `scan_table` paths).
    #[getter]
    fn issues<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        match &self.issues_batch {
            None => Ok(py.None().into_bound(py)),
            Some(batch) => {
                // Convert `RecordBatch` → `pyarrow.RecordBatch` via
                // the Arrow C Data Interface. Then wrap into a
                // `pyarrow.Table` (single-chunk) so the public API
                // contract is `pyarrow.Table` as documented in
                // `docs/python-roadmap.md`.
                let rb_py = batch.to_pyarrow(py)?;
                let pa = py.import_bound("pyarrow")?;
                let batches = PyList::new_bound(py, [rb_py]);
                let table = pa.getattr("Table")?.call_method1("from_batches", (batches,))?;
                Ok(table)
            }
        }
    }

    /// The canonical-model records as a `pyarrow.Table`, present
    /// when the caller passed `normalize=True` (otherwise `None`).
    /// Schema matches `opendqi_io::{emir_schema, sftr_schema}` —
    /// the same as the on-disk Parquet output of
    /// `opendqi {emir,sftr} normalize`.
    #[getter]
    fn normalized<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        match &self.normalized_batch {
            None => Ok(py.None().into_bound(py)),
            Some(batch) => {
                let rb_py = batch.to_pyarrow(py)?;
                let pa = py.import_bound("pyarrow")?;
                let batches = PyList::new_bound(py, [rb_py]);
                let table = pa.getattr("Table")?.call_method1("from_batches", (batches,))?;
                Ok(table)
            }
        }
    }

    fn __repr__(&self) -> String {
        let issues_n = self.issues_batch.as_ref().map(|b| b.num_rows()).unwrap_or(0);
        format!(
            "PyScanResult(regime={:?}, files={}, records={}, issues={}, score={:.2})",
            self.summary.regime,
            self.summary.files_processed,
            self.summary.records_processed,
            issues_n,
            self.summary.quality_score,
        )
    }
}

/// Build a `PyDict` mirroring the canonical `ScanSummary` shape —
/// identical key set to the on-disk `summary.json`.
fn summary_to_dict<'py>(py: Python<'py>, s: &ScanSummary) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new_bound(py);
    d.set_item("regime", s.regime.to_string())?;
    d.set_item("files_processed", s.files_processed)?;
    d.set_item("records_processed", s.records_processed)?;
    d.set_item("issues_total", s.issues_total)?;

    let sev = PyDict::new_bound(py);
    for (k, v) in &s.issues_by_severity {
        sev.set_item(k.to_string(), *v)?;
    }
    d.set_item("issues_by_severity", sev)?;

    let dim = PyDict::new_bound(py);
    for (k, v) in &s.issues_by_dimension {
        dim.set_item(k.to_string(), *v)?;
    }
    d.set_item("issues_by_dimension", dim)?;

    d.set_item("quality_score", s.quality_score)?;
    d.set_item("started_at", s.started_at.to_rfc3339())?;
    d.set_item("finished_at", s.finished_at.to_rfc3339())?;
    Ok(d)
}
