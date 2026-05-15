//! OpenDQI history store — local SQLite-backed persistence of scanned
//! EMIR / SFTR records, used by lifecycle checks that need access to
//! records from prior scans (e.g. MODI without a previous NEWT,
//! duplicate NEWT for a UTI, valuation regression).
//!
//! The store is opt-in: `opendqi {emir,sftr} scan --store <path>`
//! enables it. Without the flag, OpenDQI runs entirely in-memory and
//! never touches a database.
//!
//! ## File format
//!
//! A single SQLite file. The schema is created on first open via
//! idempotent `CREATE TABLE IF NOT EXISTS` statements run through the
//! versioned migration pipeline.

use std::path::Path;

use rusqlite::Connection;

mod error;
mod load;
mod migrations;
mod persist;

pub use error::StoreError;
pub use load::FeedbackRow;

/// Open (or create) the SQLite store at `path` and run the versioned
/// migration pipeline. Existing databases (created before the
/// migration tooling existed) are detected and back-filled at v1.
/// The migration model lives in the crate-private `migrations`
/// module. The returned [`Store`] owns the connection.
pub fn open_store(path: &Path) -> Result<Store, StoreError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL").ok();
    conn.pragma_update(None, "foreign_keys", "ON")?;
    migrations::migrate(&conn)?;
    Ok(Store { conn })
}

/// Owned SQLite connection plus the OpenDQI-specific persist / load
/// methods.
pub struct Store {
    pub(crate) conn: Connection,
}
