//! `opendqi.emir` submodule — EMIR-side bindings.
//!
//! In P1 this is a placeholder that only registers the submodule;
//! `scan_parquet`, `scan_table` and `parse_xml` are added in P2 /
//! P3 / P4 commits of the v0.12 chantier (see
//! `docs/python-roadmap.md`).

use pyo3::prelude::*;

/// Register the `opendqi.emir` submodule on the parent
/// `opendqi` module.
pub fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = parent.py();
    let m = PyModule::new_bound(py, "emir")?;
    m.add("__doc__", "EMIR data-quality bindings.")?;
    parent.add_submodule(&m)?;
    Ok(())
}
