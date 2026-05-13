//! Persist a batch of records into the history store.
//!
//! Each call to `persist_emir_batch` / `persist_sftr_batch` opens a
//! single transaction, inserts a new `scans` row and one record per
//! input, returning the freshly-allocated `scan_id`.

use chrono::{DateTime, NaiveDate, Utc};
use opendqi_core::{EmirRecord, SftrRecord};
use rusqlite::{params, Transaction};

use crate::error::StoreError;
use crate::Store;

impl Store {
    /// Persist `records` as a new EMIR scan. Returns the freshly
    /// inserted `scan_id`.
    pub fn persist_emir_batch(
        &mut self,
        file_count: usize,
        records: &[EmirRecord],
    ) -> Result<i64, StoreError> {
        let now = Utc::now().timestamp();
        let tx = self.conn.transaction()?;
        let scan_id = insert_scan(&tx, "EMIR", now, file_count, records.len())?;
        for r in records {
            insert_emir_record(&tx, scan_id, now, r)?;
        }
        tx.commit()?;
        Ok(scan_id)
    }

    /// Persist `records` as a new SFTR scan. Returns the freshly
    /// inserted `scan_id`.
    pub fn persist_sftr_batch(
        &mut self,
        file_count: usize,
        records: &[SftrRecord],
    ) -> Result<i64, StoreError> {
        let now = Utc::now().timestamp();
        let tx = self.conn.transaction()?;
        let scan_id = insert_scan(&tx, "SFTR", now, file_count, records.len())?;
        for r in records {
            insert_sftr_record(&tx, scan_id, now, r)?;
        }
        tx.commit()?;
        Ok(scan_id)
    }
}

fn insert_scan(
    tx: &Transaction<'_>,
    regime: &str,
    started_at: i64,
    file_count: usize,
    record_count: usize,
) -> Result<i64, StoreError> {
    tx.execute(
        "INSERT INTO scans (regime, started_at, file_count, record_count) VALUES (?1, ?2, ?3, ?4)",
        params![regime, started_at, file_count as i64, record_count as i64],
    )?;
    Ok(tx.last_insert_rowid())
}

fn insert_emir_record(
    tx: &Transaction<'_>,
    scan_id: i64,
    ingested_at: i64,
    r: &EmirRecord,
) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO emir_records (
            scan_id, record_id, source_file, uti, prior_uti,
            action_type, event_type,
            execution_timestamp, event_timestamp, reporting_timestamp,
            effective_date, maturity_date, termination_date,
            valuation_amount, valuation_timestamp, valuation_currency,
            ingested_at
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5,
            ?6, ?7,
            ?8, ?9, ?10,
            ?11, ?12, ?13,
            ?14, ?15, ?16,
            ?17
        )",
        params![
            scan_id,
            r.record_id,
            r.source_file,
            r.uti,
            r.prior_uti,
            r.action_type,
            r.event_type,
            ts_opt(r.execution_timestamp),
            ts_opt(r.event_timestamp),
            ts_opt(r.reporting_timestamp),
            date_opt(r.effective_date),
            date_opt(r.maturity_date),
            date_opt(r.termination_date),
            r.valuation_amount.map(|d| d.to_string()),
            ts_opt(r.valuation_timestamp),
            r.valuation_currency,
            ingested_at,
        ],
    )?;
    Ok(())
}

fn insert_sftr_record(
    tx: &Transaction<'_>,
    scan_id: i64,
    ingested_at: i64,
    r: &SftrRecord,
) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO sftr_records (
            scan_id, record_id, source_file, uti, prior_uti,
            action_type, event_type,
            execution_timestamp, event_timestamp, reporting_timestamp,
            effective_date, maturity_date, termination_date,
            settlement_date, sft_type,
            ingested_at
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5,
            ?6, ?7,
            ?8, ?9, ?10,
            ?11, ?12, ?13,
            ?14, ?15,
            ?16
        )",
        params![
            scan_id,
            r.record_id,
            r.source_file,
            r.uti,
            r.prior_uti,
            r.action_type,
            r.event_type,
            ts_opt(r.execution_timestamp),
            ts_opt(r.event_timestamp),
            ts_opt(r.reporting_timestamp),
            date_opt(r.effective_date),
            date_opt(r.maturity_date),
            date_opt(r.termination_date),
            date_opt(r.settlement_date),
            r.sft_type,
            ingested_at,
        ],
    )?;
    Ok(())
}

fn ts_opt(t: Option<DateTime<Utc>>) -> Option<i64> {
    t.map(|d| d.timestamp())
}

fn date_opt(d: Option<NaiveDate>) -> Option<String> {
    d.map(|d| d.format("%Y-%m-%d").to_string())
}
