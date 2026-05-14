//! Persist a batch of records into the history store.
//!
//! Each call to `persist_emir_batch` / `persist_sftr_batch` opens a
//! single transaction, inserts a new `scans` row and one record per
//! input, returning the freshly-allocated `scan_id`.

use chrono::{DateTime, NaiveDate, Utc};
use opendqi_core::{
    EmirRecord, FeedbackRecord, FeedbackType, MarginActivityRecord, MarginStateRecord,
    ReconciliationRecord, Regime, SftrRecord, SftrTrStateRecord, TrStateRecord,
};
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

    /// Persist a batch of TR feedback records into the `feedbacks`
    /// table. All rows start with `status='open'`. Returns the number
    /// of rows inserted.
    pub fn persist_feedback_batch(
        &mut self,
        records: &[FeedbackRecord],
    ) -> Result<usize, StoreError> {
        let now = Utc::now().timestamp();
        let tx = self.conn.transaction()?;
        let mut inserted = 0usize;
        for r in records {
            insert_feedback(&tx, now, r)?;
            inserted += 1;
        }
        tx.commit()?;
        Ok(inserted)
    }

    /// Persist a batch of TR reconciliation records into the
    /// `reconciliations` table. Returns the number of rows inserted.
    pub fn persist_reconciliation_batch(
        &mut self,
        records: &[ReconciliationRecord],
    ) -> Result<usize, StoreError> {
        let now = Utc::now().timestamp();
        let tx = self.conn.transaction()?;
        let mut inserted = 0usize;
        for r in records {
            insert_reconciliation(&tx, now, r)?;
            inserted += 1;
        }
        tx.commit()?;
        Ok(inserted)
    }

    /// Persist a batch of EMIR TSR records (`auth.107`). Returns the
    /// freshly inserted `scan_id`.
    pub fn persist_tr_state_batch(
        &mut self,
        file_count: usize,
        records: &[TrStateRecord],
    ) -> Result<i64, StoreError> {
        let now = Utc::now().timestamp();
        let tx = self.conn.transaction()?;
        let scan_id = insert_scan(&tx, "EMIR", now, file_count, records.len())?;
        for r in records {
            insert_tr_state_record(&tx, scan_id, now, r)?;
        }
        tx.commit()?;
        Ok(scan_id)
    }

    /// Persist a batch of SFTR TSR records (`auth.079`). Returns the
    /// freshly inserted `scan_id`.
    pub fn persist_sftr_tr_state_batch(
        &mut self,
        file_count: usize,
        records: &[SftrTrStateRecord],
    ) -> Result<i64, StoreError> {
        let now = Utc::now().timestamp();
        let tx = self.conn.transaction()?;
        let scan_id = insert_scan(&tx, "SFTR", now, file_count, records.len())?;
        for r in records {
            insert_sftr_tr_state_record(&tx, scan_id, now, r)?;
        }
        tx.commit()?;
        Ok(scan_id)
    }

    /// Persist a batch of EMIR Margin Activity records (`auth.108`).
    /// Returns the freshly inserted `scan_id`.
    pub fn persist_margin_activity_batch(
        &mut self,
        file_count: usize,
        records: &[MarginActivityRecord],
    ) -> Result<i64, StoreError> {
        let now = Utc::now().timestamp();
        let tx = self.conn.transaction()?;
        let scan_id = insert_scan(&tx, "EMIR", now, file_count, records.len())?;
        for r in records {
            insert_margin_activity_record(&tx, scan_id, now, r)?;
        }
        tx.commit()?;
        Ok(scan_id)
    }

    /// Persist a batch of EMIR Margin State records (`auth.109`).
    /// Returns the freshly inserted `scan_id`.
    pub fn persist_margin_state_batch(
        &mut self,
        file_count: usize,
        records: &[MarginStateRecord],
    ) -> Result<i64, StoreError> {
        let now = Utc::now().timestamp();
        let tx = self.conn.transaction()?;
        let scan_id = insert_scan(&tx, "EMIR", now, file_count, records.len())?;
        for r in records {
            insert_margin_state_record(&tx, scan_id, now, r)?;
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

fn insert_feedback(
    tx: &Transaction<'_>,
    ingested_at: i64,
    r: &FeedbackRecord,
) -> Result<(), StoreError> {
    let regime = match r.regime {
        Regime::Emir => "EMIR",
        Regime::Sftr => "SFTR",
    };
    let feedback_type = match r.feedback_type {
        FeedbackType::Rejected => "rejected",
        FeedbackType::Missing => "missing",
        FeedbackType::Inaccurate => "inaccurate",
        FeedbackType::ReconciliationBreak => "reconciliation_break",
    };
    tx.execute(
        "INSERT INTO feedbacks (
            scan_id, regime, uti, feedback_type,
            reason_code, reason_description, reported_field,
            source_file, feedback_timestamp,
            status, status_set_at, ingested_at
        ) VALUES (
            NULL, ?1, ?2, ?3,
            ?4, ?5, ?6,
            ?7, ?8,
            'open', ?9, ?9
        )",
        params![
            regime,
            r.uti,
            feedback_type,
            r.reason_code,
            r.reason_description,
            r.reported_field,
            r.source_file,
            ts_opt(r.feedback_timestamp),
            ingested_at,
        ],
    )?;
    Ok(())
}

fn insert_reconciliation(
    tx: &Transaction<'_>,
    ingested_at: i64,
    r: &ReconciliationRecord,
) -> Result<(), StoreError> {
    let regime = match r.regime {
        Regime::Emir => "EMIR",
        Regime::Sftr => "SFTR",
    };
    let mismatched_fields_json = serialize_string_array(&r.mismatched_fields);
    tx.execute(
        "INSERT INTO reconciliations (
            regime, uti, reporting_counterparty, other_counterparty,
            pairing_status, reconciliation_status, mismatched_fields_json,
            source_file, reconciliation_timestamp, ingested_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            regime,
            r.uti,
            r.reporting_counterparty,
            r.other_counterparty,
            r.pairing_status,
            r.reconciliation_status,
            mismatched_fields_json,
            r.source_file,
            ts_opt(r.reconciliation_timestamp),
            ingested_at,
        ],
    )?;
    Ok(())
}

/// Hand-rolled minimal JSON string-array serialiser. Avoids pulling
/// in `serde_json` just to encode a `Vec<String>` for storage.
fn serialize_string_array(values: &[String]) -> String {
    let mut out = String::with_capacity(values.len() * 16 + 2);
    out.push('[');
    for (i, v) in values.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push('"');
        for ch in v.chars() {
            match ch {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
                c => out.push(c),
            }
        }
        out.push('"');
    }
    out.push(']');
    out
}

fn insert_tr_state_record(
    tx: &Transaction<'_>,
    scan_id: i64,
    ingested_at: i64,
    r: &TrStateRecord,
) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO tr_state_records (
            scan_id, record_id, source_file, state_as_of,
            uti, reporting_counterparty, other_counterparty, status,
            notional_amount, notional_currency,
            valuation_amount, valuation_currency, valuation_timestamp,
            effective_date, maturity_date, termination_date,
            collateral_portfolio_code,
            ingested_at
        ) VALUES (
            ?1, ?2, ?3, ?4,
            ?5, ?6, ?7, ?8,
            ?9, ?10,
            ?11, ?12, ?13,
            ?14, ?15, ?16,
            ?17,
            ?18
        )",
        params![
            scan_id,
            r.record_id,
            r.source_file,
            ts_opt(r.state_as_of),
            r.uti,
            r.reporting_counterparty,
            r.other_counterparty,
            r.status,
            r.notional_amount.map(|d| d.to_string()),
            r.notional_currency,
            r.valuation_amount.map(|d| d.to_string()),
            r.valuation_currency,
            ts_opt(r.valuation_timestamp),
            date_opt(r.effective_date),
            date_opt(r.maturity_date),
            date_opt(r.termination_date),
            r.collateral_portfolio_code,
            ingested_at,
        ],
    )?;
    Ok(())
}

fn insert_sftr_tr_state_record(
    tx: &Transaction<'_>,
    scan_id: i64,
    ingested_at: i64,
    r: &SftrTrStateRecord,
) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO sftr_tr_state_records (
            scan_id, record_id, source_file, state_as_of,
            uti, reporting_counterparty, other_counterparty, status,
            sft_type,
            loan_value, loan_currency,
            collateral_value, collateral_currency, haircut, reuse_indicator,
            effective_date, maturity_date, termination_date, settlement_date,
            collateral_portfolio_code, collateral_isin,
            ingested_at
        ) VALUES (
            ?1, ?2, ?3, ?4,
            ?5, ?6, ?7, ?8,
            ?9,
            ?10, ?11,
            ?12, ?13, ?14, ?15,
            ?16, ?17, ?18, ?19,
            ?20, ?21,
            ?22
        )",
        params![
            scan_id,
            r.record_id,
            r.source_file,
            ts_opt(r.state_as_of),
            r.uti,
            r.reporting_counterparty,
            r.other_counterparty,
            r.status,
            r.sft_type,
            r.loan_value.map(|d| d.to_string()),
            r.loan_currency,
            r.collateral_value.map(|d| d.to_string()),
            r.collateral_currency,
            r.haircut.map(|d| d.to_string()),
            r.reuse_indicator.map(|b| b as i64),
            date_opt(r.effective_date),
            date_opt(r.maturity_date),
            date_opt(r.termination_date),
            date_opt(r.settlement_date),
            r.collateral_portfolio_code,
            r.collateral_isin,
            ingested_at,
        ],
    )?;
    Ok(())
}

fn insert_margin_activity_record(
    tx: &Transaction<'_>,
    scan_id: i64,
    ingested_at: i64,
    r: &MarginActivityRecord,
) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO margin_activity_records (
            scan_id, record_id, source_file,
            uti, counterparty_1, counterparty_2,
            action_type, event_type, collateral_portfolio_code,
            initial_margin_posted, initial_margin_collected,
            variation_margin_posted, variation_margin_collected,
            margin_currency, excess_collateral, collateral_haircut,
            event_timestamp, reporting_timestamp,
            ingested_at
        ) VALUES (
            ?1, ?2, ?3,
            ?4, ?5, ?6,
            ?7, ?8, ?9,
            ?10, ?11,
            ?12, ?13,
            ?14, ?15, ?16,
            ?17, ?18,
            ?19
        )",
        params![
            scan_id,
            r.record_id,
            r.source_file,
            r.uti,
            r.counterparty_1,
            r.counterparty_2,
            r.action_type,
            r.event_type,
            r.collateral_portfolio_code,
            r.initial_margin_posted.map(|d| d.to_string()),
            r.initial_margin_collected.map(|d| d.to_string()),
            r.variation_margin_posted.map(|d| d.to_string()),
            r.variation_margin_collected.map(|d| d.to_string()),
            r.margin_currency,
            r.excess_collateral.map(|d| d.to_string()),
            r.collateral_haircut.map(|d| d.to_string()),
            ts_opt(r.event_timestamp),
            ts_opt(r.reporting_timestamp),
            ingested_at,
        ],
    )?;
    Ok(())
}

fn insert_margin_state_record(
    tx: &Transaction<'_>,
    scan_id: i64,
    ingested_at: i64,
    r: &MarginStateRecord,
) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO margin_state_records (
            scan_id, record_id, source_file,
            uti, counterparty_1, counterparty_2, collateral_portfolio_code,
            initial_margin_posted_current, initial_margin_collected_current,
            variation_margin_posted_current, variation_margin_collected_current,
            margin_currency, collateral_market_value, haircut_applied,
            collateralization_category, state_as_of,
            ingested_at
        ) VALUES (
            ?1, ?2, ?3,
            ?4, ?5, ?6, ?7,
            ?8, ?9,
            ?10, ?11,
            ?12, ?13, ?14,
            ?15, ?16,
            ?17
        )",
        params![
            scan_id,
            r.record_id,
            r.source_file,
            r.uti,
            r.counterparty_1,
            r.counterparty_2,
            r.collateral_portfolio_code,
            r.initial_margin_posted_current.map(|d| d.to_string()),
            r.initial_margin_collected_current.map(|d| d.to_string()),
            r.variation_margin_posted_current.map(|d| d.to_string()),
            r.variation_margin_collected_current.map(|d| d.to_string()),
            r.margin_currency,
            r.collateral_market_value.map(|d| d.to_string()),
            r.haircut_applied.map(|d| d.to_string()),
            r.collateralization_category,
            ts_opt(r.state_as_of),
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
