//! `anyhow::Error` → `PyErr` mapping.
//!
//! Every public OpenDQI Rust function returns `anyhow::Result<T>`.
//! At the PyO3 boundary we surface the error as a generic
//! `RuntimeError` whose message is the full `anyhow` chain
//! (preserving the `with_context` annotations engineers added on
//! the Rust side). Specialised error types can be added later
//! without breaking this contract.

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

/// Convert any `anyhow::Error` into a Python `RuntimeError`,
/// flattening the cause chain into the message so the Python
/// traceback reproduces the Rust diagnostic.
pub fn to_py_err(err: anyhow::Error) -> PyErr {
    // `{:#}` prints the full anyhow chain on a single line
    // (`{:?}` would include the backtrace which is too noisy
    // for a Python tracebacks).
    PyRuntimeError::new_err(format!("{:#}", err))
}
