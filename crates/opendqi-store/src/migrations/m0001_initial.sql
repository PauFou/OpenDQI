-- Migration v1 — initial OpenDQI history store schema.
-- Idempotent (CREATE TABLE IF NOT EXISTS / CREATE INDEX IF NOT EXISTS)
-- so a re-run on an already-migrated database is a safe no-op. The
-- migration pipeline records this version in the `schema_version`
-- table once applied; future migrations append rows there.

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

CREATE TABLE IF NOT EXISTS reconciliations (
    recon_id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    regime                    TEXT    NOT NULL CHECK(regime IN ('EMIR','SFTR')),
    uti                       TEXT,
    reporting_counterparty    TEXT,
    other_counterparty        TEXT,
    pairing_status            TEXT,
    reconciliation_status     TEXT,
    mismatched_fields_json    TEXT,
    source_file               TEXT,
    reconciliation_timestamp  INTEGER,
    ingested_at               INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS reconciliations_uti ON reconciliations(uti);

CREATE TABLE IF NOT EXISTS tr_state_records (
    scan_id                    INTEGER NOT NULL REFERENCES scans(scan_id),
    record_id                  TEXT,
    source_file                TEXT,
    state_as_of                INTEGER,
    uti                        TEXT,
    reporting_counterparty     TEXT,
    other_counterparty         TEXT,
    status                     TEXT,
    notional_amount            TEXT,
    notional_currency          TEXT,
    valuation_amount           TEXT,
    valuation_currency         TEXT,
    valuation_timestamp        INTEGER,
    effective_date             TEXT,
    maturity_date              TEXT,
    termination_date           TEXT,
    collateral_portfolio_code  TEXT,
    ingested_at                INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS tr_state_records_uti ON tr_state_records(uti);

CREATE TABLE IF NOT EXISTS sftr_tr_state_records (
    scan_id                    INTEGER NOT NULL REFERENCES scans(scan_id),
    record_id                  TEXT,
    source_file                TEXT,
    state_as_of                INTEGER,
    uti                        TEXT,
    reporting_counterparty     TEXT,
    other_counterparty         TEXT,
    status                     TEXT,
    sft_type                   TEXT,
    loan_value                 TEXT,
    loan_currency              TEXT,
    collateral_value           TEXT,
    collateral_currency        TEXT,
    haircut                    TEXT,
    reuse_indicator            INTEGER,
    effective_date             TEXT,
    maturity_date              TEXT,
    termination_date           TEXT,
    settlement_date            TEXT,
    collateral_portfolio_code  TEXT,
    collateral_isin            TEXT,
    ingested_at                INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS sftr_tr_state_records_uti ON sftr_tr_state_records(uti);

CREATE TABLE IF NOT EXISTS margin_activity_records (
    scan_id                       INTEGER NOT NULL REFERENCES scans(scan_id),
    record_id                     TEXT,
    source_file                   TEXT,
    uti                           TEXT,
    counterparty_1                TEXT,
    counterparty_2                TEXT,
    action_type                   TEXT,
    event_type                    TEXT,
    collateral_portfolio_code     TEXT,
    initial_margin_posted         TEXT,
    initial_margin_collected      TEXT,
    variation_margin_posted       TEXT,
    variation_margin_collected    TEXT,
    margin_currency               TEXT,
    excess_collateral             TEXT,
    collateral_haircut            TEXT,
    event_timestamp               INTEGER,
    reporting_timestamp           INTEGER,
    ingested_at                   INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS margin_activity_records_portfolio
    ON margin_activity_records(collateral_portfolio_code);

CREATE TABLE IF NOT EXISTS margin_state_records (
    scan_id                              INTEGER NOT NULL REFERENCES scans(scan_id),
    record_id                            TEXT,
    source_file                          TEXT,
    uti                                  TEXT,
    counterparty_1                       TEXT,
    counterparty_2                       TEXT,
    collateral_portfolio_code            TEXT,
    initial_margin_posted_current        TEXT,
    initial_margin_collected_current     TEXT,
    variation_margin_posted_current      TEXT,
    variation_margin_collected_current   TEXT,
    margin_currency                      TEXT,
    collateral_market_value              TEXT,
    haircut_applied                      TEXT,
    collateralization_category           TEXT,
    state_as_of                          INTEGER,
    ingested_at                          INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS margin_state_records_uti ON margin_state_records(uti);
CREATE INDEX IF NOT EXISTS margin_state_records_portfolio
    ON margin_state_records(collateral_portfolio_code);
