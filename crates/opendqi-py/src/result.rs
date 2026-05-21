//! `PyScanResult` — the Python-facing result object returned by
//! every `opendqi.{emir,sftr}.scan_*` function.
//!
//! P3: `issues` is now a real Arrow `pyarrow.Table` (the v1.0
//! contract). `normalized` is still `None` until P5.

use std::path::Path;

use arrow::array::RecordBatch;
use arrow::pyarrow::ToPyArrow;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyList};

use opendqi_core::{DqIssue, DqiEvidence, DqiIndicator, ScanSummary};

use crate::errors::to_py_err;

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
                let table = pa
                    .getattr("Table")?
                    .call_method1("from_batches", (batches,))?;
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
                let table = pa
                    .getattr("Table")?
                    .call_method1("from_batches", (batches,))?;
                Ok(table)
            }
        }
    }

    fn __repr__(&self) -> String {
        let issues_n = self
            .issues_batch
            .as_ref()
            .map(|b| b.num_rows())
            .unwrap_or(0);
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

/// v0.15 Data Quality Pack result exposed to Python — bundles
/// the 10 [`DqiIndicator`] rows, their [`DqiEvidence`] (≤ 20 per
/// indicator), the granular issue stream from running the
/// existing 216-check registries, and a standard [`ScanSummary`]
/// over those issues.
///
/// Getters all return `pyarrow.Table`s (single-chunk) matching
/// the v1.0 stable schemas in
/// [`crate::dqi_schemas::indicators_schema`] /
/// [`crate::dqi_schemas::evidence_schema`] /
/// [`crate::issues::issues_schema`]. `.summary` returns a dict
/// with the same shape as `summary.json`. `.report(out_dir)`
/// writes the same 5 files the CLI subcommand does.
#[pyclass(module = "opendqi", frozen)]
pub struct PyDqiPackResult {
    pub(crate) indicators: Vec<DqiIndicator>,
    pub(crate) evidence: Vec<DqiEvidence>,
    pub(crate) issues: Vec<DqIssue>,
    pub(crate) summary: ScanSummary,
    pub(crate) indicators_batch: RecordBatch,
    pub(crate) evidence_batch: RecordBatch,
    pub(crate) issues_batch: RecordBatch,
}

impl PyDqiPackResult {
    pub(crate) fn new(
        indicators: Vec<DqiIndicator>,
        evidence: Vec<DqiEvidence>,
        issues: Vec<DqIssue>,
        summary: ScanSummary,
        indicators_batch: RecordBatch,
        evidence_batch: RecordBatch,
        issues_batch: RecordBatch,
    ) -> Self {
        Self {
            indicators,
            evidence,
            issues,
            summary,
            indicators_batch,
            evidence_batch,
            issues_batch,
        }
    }
}

#[pymethods]
impl PyDqiPackResult {
    /// Indicators as `pyarrow.Table` — v1.0 stable 11-column
    /// schema (see `crate::dqi_schemas::indicators_schema`).
    #[getter]
    fn indicators<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        batch_to_pyarrow_table(py, &self.indicators_batch)
    }

    /// Evidence rows as `pyarrow.Table` — v1.0 stable 7-column
    /// schema (see `crate::dqi_schemas::evidence_schema`).
    #[getter]
    fn evidence<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        batch_to_pyarrow_table(py, &self.evidence_batch)
    }

    /// Granular issues from running the existing 216-check
    /// registries on every provided layer — same `pyarrow.Table`
    /// 11-column contract as the v0.12+ `PyScanResult.issues`.
    #[getter]
    fn issues<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        batch_to_pyarrow_table(py, &self.issues_batch)
    }

    /// ScanSummary as a dict — same shape as the on-disk
    /// `summary.json` (the DQI pack reuses the existing
    /// `IssueAggregator` contract).
    #[getter]
    fn summary<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        summary_to_dict(py, &self.summary)
    }

    /// Write the 5 on-disk artefacts the CLI subcommand writes:
    /// `report.html`, `summary.json`, `issues.csv`,
    /// `indicators.csv`, `evidence.csv`. `out_dir` is created
    /// if absent.
    fn report(&self, out_dir: &str) -> PyResult<()> {
        let out = Path::new(out_dir);
        std::fs::create_dir_all(out)
            .map_err(|e| to_py_err(anyhow::anyhow!("creating {out_dir}: {e}")))?;
        opendqi_report::write_summary_json(&out.join("summary.json"), &self.summary)
            .map_err(to_py_err)?;
        opendqi_report::write_issues_csv(&out.join("issues.csv"), &self.issues)
            .map_err(to_py_err)?;
        opendqi_report::write_indicators_csv(&out.join("indicators.csv"), &self.indicators)
            .map_err(to_py_err)?;
        opendqi_report::write_evidence_csv(&out.join("evidence.csv"), &self.evidence)
            .map_err(to_py_err)?;
        opendqi_report::write_report_html_with_dqi(
            &out.join("report.html"),
            &self.summary,
            &self.issues,
            &[],
            Some(&self.indicators),
        )
        .map_err(to_py_err)?;
        Ok(())
    }

    fn __repr__(&self) -> String {
        let computed = self
            .indicators
            .iter()
            .filter(|i| !matches!(i.status, opendqi_core::DqiStatus::NotApplicable))
            .count();
        format!(
            "PyDqiPackResult(indicators={}/{} computed, evidence={}, issues={}, score={:.2})",
            computed,
            self.indicators.len(),
            self.evidence.len(),
            self.issues.len(),
            self.summary.quality_score,
        )
    }
}

/// Wrap an Arrow `RecordBatch` as a single-chunk `pyarrow.Table`.
/// Shared with the [`PyScanResult`] getters.
fn batch_to_pyarrow_table<'py>(
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
