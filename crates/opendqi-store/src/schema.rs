//! SQLite schema for the OpenDQI history store. Idempotent migrations
//! run on every `open_store` call — no separate migration tooling
//! needed for the v1 schema.

use rusqlite::Connection;

use crate::error::StoreError;

pub(crate) fn migrate(conn: &Connection) -> Result<(), StoreError> {
    conn.execute_batch(SCHEMA_V1)?;
    Ok(())
}

const SCHEMA_V1: &str = r#"
CREATE TABLE IF NOT EXISTS scans (
    scan_id        INTEGER PRIMARY KEY AUTOINCREMENT,
    regime         TEXT    NOT NULL CHECK(regime IN ('EMIR','SFTR')),
    started_at     INTEGER NOT NULL,
    file_count     INTEGER NOT NULL,
    record_count   INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS emir_records (
    scan_id              INTEGER NOT NULL REFERENCES scans(scan_id),
    record_id            TEXT,
    source_file          TEXT,
    uti                  TEXT,
    prior_uti            TEXT,
    action_type          TEXT,
    event_type           TEXT,
    execution_timestamp  INTEGER,
    event_timestamp      INTEGER,
    reporting_timestamp  INTEGER,
    effective_date       TEXT,
    maturity_date        TEXT,
    termination_date     TEXT,
    valuation_amount     TEXT,
    valuation_timestamp  INTEGER,
    valuation_currency   TEXT,
    ingested_at          INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS emir_records_uti     ON emir_records(uti);
CREATE INDEX IF NOT EXISTS emir_records_uti_act ON emir_records(uti, action_type);

CREATE TABLE IF NOT EXISTS sftr_records (
    scan_id              INTEGER NOT NULL REFERENCES scans(scan_id),
    record_id            TEXT,
    source_file          TEXT,
    uti                  TEXT,
    prior_uti            TEXT,
    action_type          TEXT,
    event_type           TEXT,
    execution_timestamp  INTEGER,
    event_timestamp      INTEGER,
    reporting_timestamp  INTEGER,
    effective_date       TEXT,
    maturity_date        TEXT,
    termination_date     TEXT,
    settlement_date      TEXT,
    sft_type             TEXT,
    ingested_at          INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS sftr_records_uti     ON sftr_records(uti);
CREATE INDEX IF NOT EXISTS sftr_records_uti_act ON sftr_records(uti, action_type);

CREATE TABLE IF NOT EXISTS feedbacks (
    feedback_id          INTEGER PRIMARY KEY AUTOINCREMENT,
    scan_id              INTEGER,
    regime               TEXT    NOT NULL CHECK(regime IN ('EMIR','SFTR')),
    uti                  TEXT,
    feedback_type        TEXT    NOT NULL,
    reason_code          TEXT,
    reason_description   TEXT,
    reported_field       TEXT,
    source_file          TEXT,
    feedback_timestamp   INTEGER,
    status               TEXT    NOT NULL DEFAULT 'open' CHECK(status IN ('open','resolved','stale')),
    status_set_at        INTEGER NOT NULL,
    ingested_at          INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS feedbacks_uti_status ON feedbacks(uti, status);
CREATE INDEX IF NOT EXISTS feedbacks_status     ON feedbacks(status);
"#;
