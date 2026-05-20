//! `opendqi.sftr` submodule — SFTR-side bindings (symmetric to `emir`).

use std::path::Path;
use std::sync::Mutex;

use chrono::Utc;
use pyo3::prelude::*;

use opendqi_core::dq::{default_sftr_checks, stream_checks_into, CheckContext};
use opendqi_core::{Regime, SortedIssueSink, Thresholds};

use crate::errors::to_py_err;
use crate::result::PyScanResult;

const STREAM_SPILL_MAX_ISSUES: usize = 65_536;

/// `opendqi.sftr.scan_parquet(path: str) -> PyScanResult`.
///
/// Reads the canonical SFTR Parquet at `path`, runs the 44
/// single-batch SFTR data-quality checks (`default_sftr_checks()`),
/// and returns a `PyScanResult` whose `.summary` dict is identical
/// in shape to the on-disk `summary.json` produced by
/// `opendqi sftr scan ... --out`.
#[pyfunction]
pub fn scan_parquet(path: &str) -> PyResult<PyScanResult> {
    let started_at = Utc::now();

    let records = opendqi_io::read_sftr_parquet(Path::new(path)).map_err(to_py_err)?;

    let ctx = CheckContext::now_with_defaults();
    let checks = default_sftr_checks();
    let thresholds = Thresholds::default();
    let sink = Mutex::new(SortedIssueSink::new(&thresholds, STREAM_SPILL_MAX_ISSUES));
    // SFTR uses the generic `stream_checks_into` with a closure
    // that captures `&records` and `&ctx` (mirror of
    // `stream_emir_checks_into`'s body, but for `SftrCheck`).
    stream_checks_into(&checks, &sink, |c| c.run(&records, &ctx));

    let finished_at = Utc::now();
    let n = records.len() as u32;
    let (summary, sorted_issues) = sink
        .into_inner()
        .expect("sink mutex not poisoned")
        .finish(Regime::Sftr, 1, n, started_at, finished_at);

    let batch = crate::issues::issues_to_record_batch(sorted_issues).map_err(to_py_err)?;
    Ok(PyScanResult::new(summary, Some(batch)))
}

/// Register the `opendqi.sftr` submodule on the parent
/// `opendqi` module.
pub fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = parent.py();
    let m = PyModule::new_bound(py, "sftr")?;
    m.add("__doc__", "SFTR data-quality bindings.")?;
    m.add_function(wrap_pyfunction!(scan_parquet, &m)?)?;
    parent.add_submodule(&m)?;
    Ok(())
}
