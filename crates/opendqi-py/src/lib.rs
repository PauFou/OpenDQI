//! OpenDQI — Python / Arrow bindings.
//!
//! See [`docs/python-roadmap.md`](../../../docs/python-roadmap.md)
//! for the full architecture spec. P1 ships the package skeleton
//! and registers two empty submodules `opendqi.emir` and
//! `opendqi.sftr`; the actual `scan_parquet` / `scan_table` /
//! `parse_xml` functions are added in P2 → P4.
//!
//! Compiled by `maturin`, never by `cargo build --workspace` (this
//! crate is deliberately not in `[workspace] members` of the root
//! `Cargo.toml`).

#![warn(missing_docs)]

use pyo3::prelude::*;

mod convert;
mod emir;
mod errors;
mod issues;
mod result;
mod sftr;

/// The `_opendqi` Python extension module — the compiled cdylib
/// loaded from inside the `opendqi` Python package via
/// `from ._opendqi import *` (see `python/opendqi/__init__.py`).
///
/// The lib was renamed `opendqi` → `_opendqi` in v0.13.0 to
/// switch to maturin's `python-source` layout so we can ship
/// pure-Python files (e.g. `opendqi.spark`) next to the
/// compiled module. Users importing `opendqi` continue to see
/// the same surface — `opendqi.__version__`, `opendqi.emir.*`,
/// `opendqi.sftr.*` — because `python/opendqi/__init__.py`
/// re-exports everything via `from ._opendqi import *`.
#[pymodule]
fn _opendqi(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Expose the crate version as `opendqi.__version__` (mirrors
    // the standard Python convention; PyO3 has no automatic
    // version exposure).
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add(
        "__doc__",
        "OpenDQI — local-first EMIR/SFTR data quality engine, \
         Python / Arrow bindings. See \
         https://github.com/PauFou/OpenDQI for documentation.",
    )?;

    // Submodules: `opendqi.emir` and `opendqi.sftr` (via the
    // package __init__.py's re-export). Each carries its own
    // `scan_parquet` / `scan_table` / `parse_xml` functions.
    emir::register(m)?;
    sftr::register(m)?;

    Ok(())
}
