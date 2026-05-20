//! `PyScanResult` — the Python-facing result object returned by
//! every `opendqi.{emir,sftr}.scan_*` function.
//!
//! P2: only the `summary` field is populated; `issues` and
//! `normalized` are added in P3 / P5 (they stay `None` here).

use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict};

use opendqi_core::ScanSummary;

/// Result of an OpenDQI scan, exposed to Python.
///
/// Fields are populated incrementally across the v0.12 chantier :
/// - `summary`    — P2 onward (always present).
/// - `issues`     — P3 onward (Arrow `pyarrow.Table` with the
///                  v1.0 schema; `None` here in P2).
/// - `normalized` — P5 onward (Arrow `pyarrow.Table` mirroring
///                  the canonical record model; `None` here).
#[pyclass(module = "opendqi", frozen)]
pub struct PyScanResult {
    pub(crate) summary: ScanSummary,
    // P3 will replace this with `Option<RecordBatch>` (or its
    // Python proxy). Kept as a placeholder for now.
    pub(crate) issues_placeholder: (),
    pub(crate) normalized_placeholder: (),
}

impl PyScanResult {
    pub(crate) fn new(summary: ScanSummary) -> Self {
        Self {
            summary,
            issues_placeholder: (),
            normalized_placeholder: (),
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

    /// P3-pending. Returns `None` in v0.12.0-P2.
    #[getter]
    fn issues<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny> {
        py.None().into_bound(py)
    }

    /// P5-pending. Returns `None` in v0.12.0-P2.
    #[getter]
    fn normalized<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny> {
        py.None().into_bound(py)
    }

    fn __repr__(&self) -> String {
        format!(
            "PyScanResult(regime={:?}, files={}, records={}, issues={}, score={:.2})",
            self.summary.regime,
            self.summary.files_processed,
            self.summary.records_processed,
            self.summary.issues_total,
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
