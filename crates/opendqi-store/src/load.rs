//! Load prior records by UTI from the history store.
//!
//! For lifecycle checks we only need a *narrow* subset of fields per
//! prior record — enough to evaluate action / event / valuation /
//! termination conditions. We re-hydrate into the canonical
//! `EmirRecord` / `SftrRecord` types but leave non-stored columns at
//! their `Default` values.

use std::str::FromStr;

use chrono::{DateTime, NaiveDate, Utc};
use opendqi_core::{EmirRecord, SftrRecord};
use rusqlite::{params_from_iter, types::Value, OptionalExtension};
use rust_decimal::Decimal;

use crate::error::StoreError;
use crate::Store;

impl Store {
    /// Load all EMIR records that share a UTI with `current_utis` and
    /// were ingested by a prior scan (`scan_id < exclude_scan_id`).
    ///
    /// Empty `current_utis` short-circuits to an empty result.
    pub fn load_prior_emir(
        &self,
        current_utis: &[&str],
        exclude_scan_id: i64,
    ) -> Result<Vec<EmirRecord>, StoreError> {
        if current_utis.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = "?,".repeat(current_utis.len() - 1) + "?";
        let sql = format!(
            "SELECT record_id, source_file, uti, prior_uti, action_type, event_type, \
                execution_timestamp, event_timestamp, reporting_timestamp, \
                effective_date, maturity_date, termination_date, \
                valuation_amount, valuation_timestamp, valuation_currency \
             FROM emir_records \
             WHERE scan_id < ? AND uti IN ({placeholders})"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let mut bind: Vec<Value> = Vec::with_capacity(current_utis.len() + 1);
        bind.push(Value::Integer(exclude_scan_id));
        for u in current_utis {
            bind.push(Value::Text((*u).to_owned()));
        }
        let rows = stmt.query_map(params_from_iter(bind), |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<i64>>(6)?,
                row.get::<_, Option<i64>>(7)?,
                row.get::<_, Option<i64>>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, Option<String>>(10)?,
                row.get::<_, Option<String>>(11)?,
                row.get::<_, Option<String>>(12)?,
                row.get::<_, Option<i64>>(13)?,
                row.get::<_, Option<String>>(14)?,
            ))
        })?;

        let mut out = Vec::new();
        for r in rows {
            let (
                record_id,
                source_file,
                uti,
                prior_uti,
                action_type,
                event_type,
                exec_ts,
                evt_ts,
                rpt_ts,
                eff_d,
                mat_d,
                term_d,
                val_amt,
                val_ts,
                val_ccy,
            ) = r?;
            out.push(EmirRecord {
                record_id,
                source_file,
                uti,
                prior_uti,
                action_type,
                event_type,
                execution_timestamp: ts_from(exec_ts),
                event_timestamp: ts_from(evt_ts),
                reporting_timestamp: ts_from(rpt_ts),
                effective_date: date_from(eff_d)?,
                maturity_date: date_from(mat_d)?,
                termination_date: date_from(term_d)?,
                valuation_amount: decimal_from(val_amt)?,
                valuation_timestamp: ts_from(val_ts),
                valuation_currency: val_ccy,
                ..Default::default()
            });
        }
        Ok(out)
    }

    /// Load all SFTR records that share a UTI with `current_utis` and
    /// were ingested by a prior scan (`scan_id < exclude_scan_id`).
    pub fn load_prior_sftr(
        &self,
        current_utis: &[&str],
        exclude_scan_id: i64,
    ) -> Result<Vec<SftrRecord>, StoreError> {
        if current_utis.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = "?,".repeat(current_utis.len() - 1) + "?";
        let sql = format!(
            "SELECT record_id, source_file, uti, prior_uti, action_type, event_type, \
                execution_timestamp, event_timestamp, reporting_timestamp, \
                effective_date, maturity_date, termination_date, \
                settlement_date, sft_type \
             FROM sftr_records \
             WHERE scan_id < ? AND uti IN ({placeholders})"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let mut bind: Vec<Value> = Vec::with_capacity(current_utis.len() + 1);
        bind.push(Value::Integer(exclude_scan_id));
        for u in current_utis {
            bind.push(Value::Text((*u).to_owned()));
        }
        let rows = stmt.query_map(params_from_iter(bind), |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<i64>>(6)?,
                row.get::<_, Option<i64>>(7)?,
                row.get::<_, Option<i64>>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, Option<String>>(10)?,
                row.get::<_, Option<String>>(11)?,
                row.get::<_, Option<String>>(12)?,
                row.get::<_, Option<String>>(13)?,
            ))
        })?;

        let mut out = Vec::new();
        for r in rows {
            let (
                record_id,
                source_file,
                uti,
                prior_uti,
                action_type,
                event_type,
                exec_ts,
                evt_ts,
                rpt_ts,
                eff_d,
                mat_d,
                term_d,
                sttl_d,
                sft_type,
            ) = r?;
            out.push(SftrRecord {
                record_id,
                source_file,
                uti,
                prior_uti,
                action_type,
                event_type,
                execution_timestamp: ts_from(exec_ts),
                event_timestamp: ts_from(evt_ts),
                reporting_timestamp: ts_from(rpt_ts),
                effective_date: date_from(eff_d)?,
                maturity_date: date_from(mat_d)?,
                termination_date: date_from(term_d)?,
                settlement_date: date_from(sttl_d)?,
                sft_type,
                ..Default::default()
            });
        }
        Ok(out)
    }

    /// Count rows in `emir_records`. Test helper.
    #[doc(hidden)]
    pub fn count_emir(&self) -> Result<i64, StoreError> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM emir_records", [], |r| r.get(0))
            .optional()?
            .unwrap_or(0))
    }

    /// Count rows in `sftr_records`. Test helper.
    #[doc(hidden)]
    pub fn count_sftr(&self) -> Result<i64, StoreError> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM sftr_records", [], |r| r.get(0))
            .optional()?
            .unwrap_or(0))
    }
}

fn ts_from(t: Option<i64>) -> Option<DateTime<Utc>> {
    t.and_then(|s| DateTime::<Utc>::from_timestamp(s, 0))
}

fn date_from(s: Option<String>) -> Result<Option<NaiveDate>, StoreError> {
    match s {
        Some(s) => Ok(Some(NaiveDate::parse_from_str(&s, "%Y-%m-%d")?)),
        None => Ok(None),
    }
}

fn decimal_from(s: Option<String>) -> Result<Option<Decimal>, StoreError> {
    match s {
        Some(s) => Ok(Some(Decimal::from_str(&s)?)),
        None => Ok(None),
    }
}
