//! `opendqi.sftr` submodule — SFTR-side bindings (symmetric to `emir`).

use pyo3::prelude::*;

/// Register the `opendqi.sftr` submodule on the parent
/// `opendqi` module.
pub fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = parent.py();
    let m = PyModule::new_bound(py, "sftr")?;
    m.add("__doc__", "SFTR data-quality bindings.")?;
    parent.add_submodule(&m)?;
    Ok(())
}
