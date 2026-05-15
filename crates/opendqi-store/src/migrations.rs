//! Versioned SQLite migrations for the OpenDQI history store.
//!
//! Each migration is identified by a monotonic integer `version` and a
//! short `name`. The migrator records every applied version in a
//! `schema_version` table; a re-run is a no-op (idempotent). For
//! databases created before this pipeline existed — i.e. files that
//! already carry the v1 schema but have no `schema_version` table —
//! the migrator detects the legacy state and back-fills the v1 marker
//! without re-running the SQL.
//!
//! Adding a future migration:
//!
//! 1. Drop the SQL into `migrations/m000N_<short_name>.sql`.
//! 2. Append a `Migration` entry to [`MIGRATIONS`] below.
//!
//! Migrations apply in ascending `version` order, each in its own
//! transaction.

use std::collections::HashSet;

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};

use crate::error::StoreError;

pub(crate) struct Migration {
    pub version: i32,
    pub name: &'static str,
    pub sql: &'static str,
}

pub(crate) const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial_schema",
        sql: include_str!("migrations/m0001_initial.sql"),
    },
    Migration {
        version: 2,
        name: "feedback_validation_rules",
        sql: include_str!("migrations/m0002_feedback_validation_rules.sql"),
    },
];

pub(crate) fn migrate(conn: &Connection) -> Result<(), StoreError> {
    ensure_schema_version_table(conn)?;
    let mut applied = load_applied(conn)?;
    if applied.is_empty() && has_legacy_scans_table(conn)? {
        record_legacy_v1(conn)?;
        applied.insert(1);
    }
    for m in MIGRATIONS {
        if applied.contains(&m.version) {
            continue;
        }
        apply_migration(conn, m)?;
    }
    Ok(())
}

fn ensure_schema_version_table(conn: &Connection) -> Result<(), StoreError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (
            version    INTEGER PRIMARY KEY,
            name       TEXT    NOT NULL,
            applied_at INTEGER NOT NULL
        );",
    )?;
    Ok(())
}

fn load_applied(conn: &Connection) -> Result<HashSet<i32>, StoreError> {
    let mut stmt = conn.prepare("SELECT version FROM schema_version")?;
    let rows = stmt.query_map([], |row| row.get::<_, i32>(0))?;
    let mut out = HashSet::new();
    for r in rows {
        out.insert(r?);
    }
    Ok(out)
}

fn has_legacy_scans_table(conn: &Connection) -> Result<bool, StoreError> {
    let found: Option<String> = conn
        .query_row(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='scans'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    Ok(found.is_some())
}

fn record_legacy_v1(conn: &Connection) -> Result<(), StoreError> {
    conn.execute(
        "INSERT INTO schema_version (version, name, applied_at) VALUES (?1, ?2, ?3)",
        params![
            1,
            "initial_schema (legacy backfill)",
            Utc::now().timestamp(),
        ],
    )?;
    Ok(())
}

fn apply_migration(conn: &Connection, m: &Migration) -> Result<(), StoreError> {
    conn.execute("BEGIN", [])?;
    let res: Result<(), StoreError> = (|| {
        conn.execute_batch(m.sql)?;
        conn.execute(
            "INSERT INTO schema_version (version, name, applied_at) VALUES (?1, ?2, ?3)",
            params![m.version, m.name, Utc::now().timestamp()],
        )?;
        Ok(())
    })();
    match res {
        Ok(()) => {
            conn.execute("COMMIT", [])?;
            Ok(())
        }
        Err(e) => {
            let _ = conn.execute("ROLLBACK", []);
            Err(e)
        }
    }
}
