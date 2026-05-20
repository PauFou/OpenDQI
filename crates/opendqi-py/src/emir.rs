//! `opendqi.emir` submodule — EMIR-side bindings.
//!
//! P2 ships `scan_parquet(path)` returning a `PyScanResult` whose
//! `summary` mirrors `summary.json`. `issues` (P3) and `normalized`
//! (P5) come in subsequent commits.

use std::path::Path;
use std::sync::Mutex;

use chrono::Utc;
use pyo3::prelude::*;

use opendqi_core::dq::{default_checks, stream_emir_checks_into, CheckContext};
use opendqi_core::{Regime, SortedIssueSink, Thresholds};

use crate::errors::to_py_err;
use crate::result::PyScanResult;

/// Match the CLI default — small enough that every shipped golden
/// stays in the no-spill path, which is byte-identical to the
/// legacy `finalize_issues` (M0.22 invariant).
const STREAM_SPILL_MAX_ISSUES: usize = 65_536;

/// `opendqi.emir.scan_parquet(path: str) -> PyScanResult`.
///
/// Reads the canonical EMIR Parquet at `path`, runs the 89
/// single-batch EMIR data-quality checks (`default_checks()`),
/// and returns a `PyScanResult` whose `.summary` dict is identical
/// in shape to the on-disk `summary.json` produced by
/// `opendqi emir scan ... --out`.
///
/// P2 scope: `issues` and `normalized` are always `None`. They
/// are wired up in P3 / P5.
#[pyfunction]
pub fn scan_parquet(path: &str) -> PyResult<PyScanResult> {
    let started_at = Utc::now();

    let records = opendqi_io::read_emir_parquet(Path::new(path)).map_err(to_py_err)?;

    let ctx = CheckContext::now_with_defaults();
    let checks = default_checks();
    let thresholds = Thresholds::default();
    let sink = Mutex::new(SortedIssueSink::new(&thresholds, STREAM_SPILL_MAX_ISSUES));
    stream_emir_checks_into(&checks, &records, &ctx, &sink);

    let finished_at = Utc::now();
    let n = records.len() as u32;
    let (summary, _sorted_issues) = sink
        .into_inner()
        .expect("sink mutex not poisoned")
        .finish(Regime::Emir, 1, n, started_at, finished_at);

    // _sorted_issues is dropped here — P3 will hold and convert it
    // into a pyarrow.Table assigned to `result.issues`.
    Ok(PyScanResult::new(summary))
}

/// Register the `opendqi.emir` submodule on the parent
/// `opendqi` module.
pub fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = parent.py();
    let m = PyModule::new_bound(py, "emir")?;
    m.add("__doc__", "EMIR data-quality bindings.")?;
    m.add_function(wrap_pyfunction!(scan_parquet, &m)?)?;
    parent.add_submodule(&m)?;
    Ok(())
}
