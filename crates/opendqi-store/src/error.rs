//! Store error type.

use thiserror::Error;

/// Errors raised by `opendqi-store`.
#[derive(Debug, Error)]
pub enum StoreError {
    /// Underlying SQLite error (open / migrate / read / write).
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// I/O error while preparing the store path (e.g. creating the
    /// parent directory).
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// Decimal parse failure when re-hydrating a record.
    #[error("decimal: {0}")]
    Decimal(#[from] rust_decimal::Error),
    /// Date / datetime parse failure when re-hydrating a record.
    #[error("chrono: {0}")]
    Chrono(#[from] chrono::ParseError),
}
