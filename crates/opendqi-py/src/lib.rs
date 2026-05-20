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

mod emir;
mod errors;
mod sftr;

/// Re-export the boundary helper for use by `emir.rs` / `sftr.rs`
/// once those gain real functions in P2.
#[allow(unused_imports)]
pub(crate) use errors::to_py_err;

/// The `opendqi` Python module — entry point loaded by
/// `import opendqi`.
#[pymodule]
fn opendqi(m: &Bound<'_, PyModule>) -> PyResult<()> {
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

    // Submodules: `opendqi.emir` and `opendqi.sftr`. Each carries
    // its own `scan_parquet` / `scan_table` / etc. once the later
    // P2-P4 increments land.
    emir::register(m)?;
    sftr::register(m)?;

    Ok(())
}
