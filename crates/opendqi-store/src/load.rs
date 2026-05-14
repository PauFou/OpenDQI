//! Load prior records by UTI from the history store.
//!
//! For lifecycle checks we only need a *narrow* subset of fields per
//! prior record — enough to evaluate action / event / valuation /
//! termination conditions. We re-hydrate into the canonical
//! `EmirRecord` / `SftrRecord` types but leave non-stored columns at
//! their `Default` values.

use std::str::FromStr;

use chrono::{DateTime, NaiveDate, Utc};
use opendqi_core::{
    EmirRecord, MarginActivityRecord, MarginStateRecord, Regime, SftrRecord, SftrTrStateRecord,
    TrStateRecord,
};
use rusqlite::{params, params_from_iter, types::Value, OptionalExtension};
use rust_decimal::Decimal;

use crate::error::StoreError;
use crate::Store;

/// A row from the `feedbacks` table, exposed to callers (e.g. the CLI
/// `feedback list` command).
#[derive(Debug, Clone)]
pub struct FeedbackRow {
    /// SQLite auto-increment primary key.
    pub feedback_id: i64,
    /// Regulatory regime.
    pub regime: Regime,
    /// UTI of the submission the feedback refers to.
    pub uti: Option<String>,
    /// TR feedback type as stored: `rejected` / `missing` /
    /// `inaccurate` / `reconciliation_break`.
    pub feedback_type: String,
    /// TR reason code (e.g. `VAL01`).
    pub reason_code: Option<String>,
    /// TR reason description.
    pub reason_description: Option<String>,
    /// For `inaccurate` feedback: which field was flagged.
    pub reported_field: Option<String>,
    /// Source file the feedback was parsed from.
    pub source_file: Option<String>,
    /// Workflow status: `open` / `resolved` / `stale`.
    pub status: String,
    /// Unix timestamp at which `status` was last set.
    pub status_set_at: i64,
    /// Unix timestamp at which the row was first persisted.
    pub ingested_at: i64,
}

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

    /// Load all EMIR TSR records that share a UTI with `current_utis`
    /// and were ingested by a prior scan (`scan_id < exclude_scan_id`).
    pub fn load_prior_tr_state(
        &self,
        current_utis: &[&str],
        exclude_scan_id: i64,
    ) -> Result<Vec<TrStateRecord>, StoreError> {
        if current_utis.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = "?,".repeat(current_utis.len() - 1) + "?";
        let sql = format!(
            "SELECT record_id, source_file, state_as_of, uti, \
                reporting_counterparty, other_counterparty, status, \
                notional_amount, notional_currency, \
                valuation_amount, valuation_currency, valuation_timestamp, \
                effective_date, maturity_date, termination_date, \
                collateral_portfolio_code \
             FROM tr_state_records \
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
                row.get::<_, Option<i64>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, Option<String>>(10)?,
                row.get::<_, Option<i64>>(11)?,
                row.get::<_, Option<String>>(12)?,
                row.get::<_, Option<String>>(13)?,
                row.get::<_, Option<String>>(14)?,
                row.get::<_, Option<String>>(15)?,
            ))
        })?;

        let mut out = Vec::new();
        for r in rows {
            let (
                record_id,
                source_file,
                state_as_of,
                uti,
                rc,
                oc,
                status,
                notional,
                notional_ccy,
                val_amt,
                val_ccy,
                val_ts,
                eff_d,
                mat_d,
                term_d,
                portfolio,
            ) = r?;
            out.push(TrStateRecord {
                record_id,
                source_file,
                regime: Regime::Emir,
                state_as_of: ts_from(state_as_of),
                uti,
                reporting_counterparty: rc,
                other_counterparty: oc,
                status,
                notional_amount: decimal_from(notional)?,
                notional_currency: notional_ccy,
                valuation_amount: decimal_from(val_amt)?,
                valuation_currency: val_ccy,
                valuation_timestamp: ts_from(val_ts),
                effective_date: date_from(eff_d)?,
                maturity_date: date_from(mat_d)?,
                termination_date: date_from(term_d)?,
                collateral_portfolio_code: portfolio,
                ..Default::default()
            });
        }
        Ok(out)
    }

    /// Load all SFTR TSR records that share a UTI with `current_utis`
    /// and were ingested by a prior scan (`scan_id < exclude_scan_id`).
    pub fn load_prior_sftr_tr_state(
        &self,
        current_utis: &[&str],
        exclude_scan_id: i64,
    ) -> Result<Vec<SftrTrStateRecord>, StoreError> {
        if current_utis.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = "?,".repeat(current_utis.len() - 1) + "?";
        let sql = format!(
            "SELECT record_id, source_file, state_as_of, uti, \
                reporting_counterparty, other_counterparty, status, sft_type, \
                loan_value, loan_currency, collateral_value, collateral_currency, \
                haircut, reuse_indicator, \
                effective_date, maturity_date, termination_date, settlement_date, \
                collateral_portfolio_code, collateral_isin \
             FROM sftr_tr_state_records \
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
                row.get::<_, Option<i64>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, Option<String>>(10)?,
                row.get::<_, Option<String>>(11)?,
                row.get::<_, Option<String>>(12)?,
                row.get::<_, Option<i64>>(13)?,
                row.get::<_, Option<String>>(14)?,
                row.get::<_, Option<String>>(15)?,
                row.get::<_, Option<String>>(16)?,
                row.get::<_, Option<String>>(17)?,
                row.get::<_, Option<String>>(18)?,
                row.get::<_, Option<String>>(19)?,
            ))
        })?;

        let mut out = Vec::new();
        for r in rows {
            let (
                record_id,
                source_file,
                state_as_of,
                uti,
                rc,
                oc,
                status,
                sft_type,
                loan,
                loan_ccy,
                coll,
                coll_ccy,
                haircut,
                reuse,
                eff_d,
                mat_d,
                term_d,
                sttl_d,
                portfolio,
                isin,
            ) = r?;
            out.push(SftrTrStateRecord {
                record_id,
                source_file,
                regime: Regime::Sftr,
                state_as_of: ts_from(state_as_of),
                uti,
                reporting_counterparty: rc,
                other_counterparty: oc,
                status,
                sft_type,
                loan_value: decimal_from(loan)?,
                loan_currency: loan_ccy,
                collateral_value: decimal_from(coll)?,
                collateral_currency: coll_ccy,
                haircut: decimal_from(haircut)?,
                reuse_indicator: reuse.map(|i| i != 0),
                effective_date: date_from(eff_d)?,
                maturity_date: date_from(mat_d)?,
                termination_date: date_from(term_d)?,
                settlement_date: date_from(sttl_d)?,
                collateral_portfolio_code: portfolio,
                collateral_isin: isin,
                ..Default::default()
            });
        }
        Ok(out)
    }

    /// Load all EMIR Margin Activity records whose
    /// `collateral_portfolio_code` matches `current_portfolios` and
    /// that were ingested by a prior scan
    /// (`scan_id < exclude_scan_id`).
    pub fn load_prior_margin_activity(
        &self,
        current_portfolios: &[&str],
        exclude_scan_id: i64,
    ) -> Result<Vec<MarginActivityRecord>, StoreError> {
        if current_portfolios.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = "?,".repeat(current_portfolios.len() - 1) + "?";
        let sql = format!(
            "SELECT record_id, source_file, uti, counterparty_1, counterparty_2, \
                action_type, event_type, collateral_portfolio_code, \
                initial_margin_posted, initial_margin_collected, \
                variation_margin_posted, variation_margin_collected, \
                margin_currency, excess_collateral, collateral_haircut, \
                event_timestamp, reporting_timestamp \
             FROM margin_activity_records \
             WHERE scan_id < ? AND collateral_portfolio_code IN ({placeholders})"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let mut bind: Vec<Value> = Vec::with_capacity(current_portfolios.len() + 1);
        bind.push(Value::Integer(exclude_scan_id));
        for p in current_portfolios {
            bind.push(Value::Text((*p).to_owned()));
        }
        let rows = stmt.query_map(params_from_iter(bind), |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, Option<String>>(10)?,
                row.get::<_, Option<String>>(11)?,
                row.get::<_, Option<String>>(12)?,
                row.get::<_, Option<String>>(13)?,
                row.get::<_, Option<String>>(14)?,
                row.get::<_, Option<i64>>(15)?,
                row.get::<_, Option<i64>>(16)?,
            ))
        })?;

        let mut out = Vec::new();
        for r in rows {
            let (
                record_id,
                source_file,
                uti,
                c1,
                c2,
                action,
                event,
                portfolio,
                im_p,
                im_c,
                vm_p,
                vm_c,
                ccy,
                xs,
                hcut,
                evt_ts,
                rpt_ts,
            ) = r?;
            out.push(MarginActivityRecord {
                record_id,
                source_file,
                regime: Regime::Emir,
                uti,
                counterparty_1: c1,
                counterparty_2: c2,
                action_type: action,
                event_type: event,
                collateral_portfolio_code: portfolio,
                initial_margin_posted: decimal_from(im_p)?,
                initial_margin_collected: decimal_from(im_c)?,
                variation_margin_posted: decimal_from(vm_p)?,
                variation_margin_collected: decimal_from(vm_c)?,
                margin_currency: ccy,
                excess_collateral: decimal_from(xs)?,
                collateral_haircut: decimal_from(hcut)?,
                event_timestamp: ts_from(evt_ts),
                reporting_timestamp: ts_from(rpt_ts),
                ..Default::default()
            });
        }
        Ok(out)
    }

    /// Load all EMIR Margin State records that share a UTI with
    /// `current_utis` and that were ingested by a prior scan.
    pub fn load_prior_margin_state(
        &self,
        current_utis: &[&str],
        exclude_scan_id: i64,
    ) -> Result<Vec<MarginStateRecord>, StoreError> {
        if current_utis.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = "?,".repeat(current_utis.len() - 1) + "?";
        let sql = format!(
            "SELECT record_id, source_file, uti, counterparty_1, counterparty_2, \
                collateral_portfolio_code, \
                initial_margin_posted_current, initial_margin_collected_current, \
                variation_margin_posted_current, variation_margin_collected_current, \
                margin_currency, collateral_market_value, haircut_applied, \
                collateralization_category, state_as_of \
             FROM margin_state_records \
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
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, Option<String>>(10)?,
                row.get::<_, Option<String>>(11)?,
                row.get::<_, Option<String>>(12)?,
                row.get::<_, Option<String>>(13)?,
                row.get::<_, Option<i64>>(14)?,
            ))
        })?;

        let mut out = Vec::new();
        for r in rows {
            let (
                record_id,
                source_file,
                uti,
                c1,
                c2,
                portfolio,
                im_p,
                im_c,
                vm_p,
                vm_c,
                ccy,
                mkt,
                hcut,
                cat,
                state_as_of,
            ) = r?;
            out.push(MarginStateRecord {
                record_id,
                source_file,
                regime: Regime::Emir,
                uti,
                counterparty_1: c1,
                counterparty_2: c2,
                collateral_portfolio_code: portfolio,
                initial_margin_posted_current: decimal_from(im_p)?,
                initial_margin_collected_current: decimal_from(im_c)?,
                variation_margin_posted_current: decimal_from(vm_p)?,
                variation_margin_collected_current: decimal_from(vm_c)?,
                margin_currency: ccy,
                collateral_market_value: decimal_from(mkt)?,
                haircut_applied: decimal_from(hcut)?,
                collateralization_category: cat,
                state_as_of: ts_from(state_as_of),
                ..Default::default()
            });
        }
        Ok(out)
    }

    /// Load every TSR row from the most recent prior scan
    /// (`scan_id < exclude_scan_id`) that produced TSR rows. Used by
    /// the cross-batch lifecycle layer to compare current vs. last
    /// snapshot — covers UTIs that have been dropped (which the
    /// per-UTI loader cannot return).
    pub fn load_latest_prior_tr_state(
        &self,
        exclude_scan_id: i64,
    ) -> Result<Vec<TrStateRecord>, StoreError> {
        let latest: Option<i64> = self
            .conn
            .query_row(
                "SELECT MAX(scan_id) FROM tr_state_records WHERE scan_id < ?1",
                params![exclude_scan_id],
                |r| r.get::<_, Option<i64>>(0),
            )
            .optional()?
            .flatten();
        let Some(scan_id) = latest else {
            return Ok(Vec::new());
        };
        let sql = "SELECT record_id, source_file, state_as_of, uti, \
                reporting_counterparty, other_counterparty, status, \
                notional_amount, notional_currency, \
                valuation_amount, valuation_currency, valuation_timestamp, \
                effective_date, maturity_date, termination_date, \
                collateral_portfolio_code \
             FROM tr_state_records WHERE scan_id = ?1";
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(params![scan_id], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<i64>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, Option<String>>(10)?,
                row.get::<_, Option<i64>>(11)?,
                row.get::<_, Option<String>>(12)?,
                row.get::<_, Option<String>>(13)?,
                row.get::<_, Option<String>>(14)?,
                row.get::<_, Option<String>>(15)?,
            ))
        })?;
        let mut out = Vec::new();
        for r in rows {
            let (rid, src, sao, uti, rc, oc, st, notional, ntccy, va, vc, vt, ed, md, td, pc) = r?;
            out.push(TrStateRecord {
                record_id: rid,
                source_file: src,
                regime: Regime::Emir,
                state_as_of: ts_from(sao),
                uti,
                reporting_counterparty: rc,
                other_counterparty: oc,
                status: st,
                notional_amount: decimal_from(notional)?,
                notional_currency: ntccy,
                valuation_amount: decimal_from(va)?,
                valuation_currency: vc,
                valuation_timestamp: ts_from(vt),
                effective_date: date_from(ed)?,
                maturity_date: date_from(md)?,
                termination_date: date_from(td)?,
                collateral_portfolio_code: pc,
                ..Default::default()
            });
        }
        Ok(out)
    }

    /// SFTR variant of [`Store::load_latest_prior_tr_state`].
    pub fn load_latest_prior_sftr_tr_state(
        &self,
        exclude_scan_id: i64,
    ) -> Result<Vec<SftrTrStateRecord>, StoreError> {
        let latest: Option<i64> = self
            .conn
            .query_row(
                "SELECT MAX(scan_id) FROM sftr_tr_state_records WHERE scan_id < ?1",
                params![exclude_scan_id],
                |r| r.get::<_, Option<i64>>(0),
            )
            .optional()?
            .flatten();
        let Some(scan_id) = latest else {
            return Ok(Vec::new());
        };
        let sql = "SELECT record_id, source_file, state_as_of, uti, \
                reporting_counterparty, other_counterparty, status, sft_type, \
                loan_value, loan_currency, collateral_value, collateral_currency, \
                haircut, reuse_indicator, \
                effective_date, maturity_date, termination_date, settlement_date, \
                collateral_portfolio_code, collateral_isin \
             FROM sftr_tr_state_records WHERE scan_id = ?1";
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(params![scan_id], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<i64>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, Option<String>>(10)?,
                row.get::<_, Option<String>>(11)?,
                row.get::<_, Option<String>>(12)?,
                row.get::<_, Option<i64>>(13)?,
                row.get::<_, Option<String>>(14)?,
                row.get::<_, Option<String>>(15)?,
                row.get::<_, Option<String>>(16)?,
                row.get::<_, Option<String>>(17)?,
                row.get::<_, Option<String>>(18)?,
                row.get::<_, Option<String>>(19)?,
            ))
        })?;
        let mut out = Vec::new();
        for r in rows {
            let (
                rid,
                src,
                sao,
                uti,
                rc,
                oc,
                st,
                sft,
                ln,
                lnc,
                cv,
                cvc,
                h,
                reu,
                ed,
                md,
                td,
                sd,
                pc,
                isin,
            ) = r?;
            out.push(SftrTrStateRecord {
                record_id: rid,
                source_file: src,
                regime: Regime::Sftr,
                state_as_of: ts_from(sao),
                uti,
                reporting_counterparty: rc,
                other_counterparty: oc,
                status: st,
                sft_type: sft,
                loan_value: decimal_from(ln)?,
                loan_currency: lnc,
                collateral_value: decimal_from(cv)?,
                collateral_currency: cvc,
                haircut: decimal_from(h)?,
                reuse_indicator: reu.map(|i| i != 0),
                effective_date: date_from(ed)?,
                maturity_date: date_from(md)?,
                termination_date: date_from(td)?,
                settlement_date: date_from(sd)?,
                collateral_portfolio_code: pc,
                collateral_isin: isin,
                ..Default::default()
            });
        }
        Ok(out)
    }

    /// MSR variant: load every margin state row from the most recent
    /// prior scan.
    pub fn load_latest_prior_margin_state(
        &self,
        exclude_scan_id: i64,
    ) -> Result<Vec<MarginStateRecord>, StoreError> {
        let latest: Option<i64> = self
            .conn
            .query_row(
                "SELECT MAX(scan_id) FROM margin_state_records WHERE scan_id < ?1",
                params![exclude_scan_id],
                |r| r.get::<_, Option<i64>>(0),
            )
            .optional()?
            .flatten();
        let Some(scan_id) = latest else {
            return Ok(Vec::new());
        };
        let sql = "SELECT record_id, source_file, uti, counterparty_1, counterparty_2, \
                collateral_portfolio_code, \
                initial_margin_posted_current, initial_margin_collected_current, \
                variation_margin_posted_current, variation_margin_collected_current, \
                margin_currency, collateral_market_value, haircut_applied, \
                collateralization_category, state_as_of \
             FROM margin_state_records WHERE scan_id = ?1";
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(params![scan_id], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, Option<String>>(10)?,
                row.get::<_, Option<String>>(11)?,
                row.get::<_, Option<String>>(12)?,
                row.get::<_, Option<String>>(13)?,
                row.get::<_, Option<i64>>(14)?,
            ))
        })?;
        let mut out = Vec::new();
        for r in rows {
            let (rid, src, uti, c1, c2, pc, ip, ic, vp, vc, ccy, mkt, hcut, cat, sao) = r?;
            out.push(MarginStateRecord {
                record_id: rid,
                source_file: src,
                regime: Regime::Emir,
                uti,
                counterparty_1: c1,
                counterparty_2: c2,
                collateral_portfolio_code: pc,
                initial_margin_posted_current: decimal_from(ip)?,
                initial_margin_collected_current: decimal_from(ic)?,
                variation_margin_posted_current: decimal_from(vp)?,
                variation_margin_collected_current: decimal_from(vc)?,
                margin_currency: ccy,
                collateral_market_value: decimal_from(mkt)?,
                haircut_applied: decimal_from(hcut)?,
                collateralization_category: cat,
                state_as_of: ts_from(sao),
                ..Default::default()
            });
        }
        Ok(out)
    }

    /// MAR variant: load every margin activity row from the most
    /// recent prior scan.
    pub fn load_latest_prior_margin_activity(
        &self,
        exclude_scan_id: i64,
    ) -> Result<Vec<MarginActivityRecord>, StoreError> {
        let latest: Option<i64> = self
            .conn
            .query_row(
                "SELECT MAX(scan_id) FROM margin_activity_records WHERE scan_id < ?1",
                params![exclude_scan_id],
                |r| r.get::<_, Option<i64>>(0),
            )
            .optional()?
            .flatten();
        let Some(scan_id) = latest else {
            return Ok(Vec::new());
        };
        let sql = "SELECT record_id, source_file, uti, counterparty_1, counterparty_2, \
                action_type, event_type, collateral_portfolio_code, \
                initial_margin_posted, initial_margin_collected, \
                variation_margin_posted, variation_margin_collected, \
                margin_currency, excess_collateral, collateral_haircut, \
                event_timestamp, reporting_timestamp \
             FROM margin_activity_records WHERE scan_id = ?1";
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(params![scan_id], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, Option<String>>(10)?,
                row.get::<_, Option<String>>(11)?,
                row.get::<_, Option<String>>(12)?,
                row.get::<_, Option<String>>(13)?,
                row.get::<_, Option<String>>(14)?,
                row.get::<_, Option<i64>>(15)?,
                row.get::<_, Option<i64>>(16)?,
            ))
        })?;
        let mut out = Vec::new();
        for r in rows {
            let (
                rid,
                src,
                uti,
                c1,
                c2,
                act,
                evt,
                pc,
                ip,
                ic,
                vp,
                vc,
                ccy,
                xs,
                hcut,
                evt_ts,
                rpt_ts,
            ) = r?;
            out.push(MarginActivityRecord {
                record_id: rid,
                source_file: src,
                regime: Regime::Emir,
                uti,
                counterparty_1: c1,
                counterparty_2: c2,
                action_type: act,
                event_type: evt,
                collateral_portfolio_code: pc,
                initial_margin_posted: decimal_from(ip)?,
                initial_margin_collected: decimal_from(ic)?,
                variation_margin_posted: decimal_from(vp)?,
                variation_margin_collected: decimal_from(vc)?,
                margin_currency: ccy,
                excess_collateral: decimal_from(xs)?,
                collateral_haircut: decimal_from(hcut)?,
                event_timestamp: ts_from(evt_ts),
                reporting_timestamp: ts_from(rpt_ts),
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

    /// List rows from the `feedbacks` table, with optional filters.
    /// All filters are AND-ed; pass `None` to skip a filter.
    pub fn list_feedbacks(
        &self,
        regime: Option<Regime>,
        uti: Option<&str>,
        status: Option<&str>,
    ) -> Result<Vec<FeedbackRow>, StoreError> {
        let mut sql = String::from(
            "SELECT feedback_id, regime, uti, feedback_type, reason_code, \
                reason_description, reported_field, source_file, status, \
                status_set_at, ingested_at \
             FROM feedbacks WHERE 1=1",
        );
        let mut bind: Vec<Value> = Vec::new();
        if let Some(r) = regime {
            sql.push_str(" AND regime = ?");
            bind.push(Value::Text(
                match r {
                    Regime::Emir => "EMIR",
                    Regime::Sftr => "SFTR",
                }
                .to_owned(),
            ));
        }
        if let Some(u) = uti {
            sql.push_str(" AND uti = ?");
            bind.push(Value::Text(u.to_owned()));
        }
        if let Some(s) = status {
            sql.push_str(" AND status = ?");
            bind.push(Value::Text(s.to_owned()));
        }
        sql.push_str(" ORDER BY feedback_id");

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(bind), |row| {
            let regime_str: String = row.get(1)?;
            Ok(FeedbackRow {
                feedback_id: row.get(0)?,
                regime: match regime_str.as_str() {
                    "SFTR" => Regime::Sftr,
                    _ => Regime::Emir,
                },
                uti: row.get(2)?,
                feedback_type: row.get(3)?,
                reason_code: row.get(4)?,
                reason_description: row.get(5)?,
                reported_field: row.get(6)?,
                source_file: row.get(7)?,
                status: row.get(8)?,
                status_set_at: row.get(9)?,
                ingested_at: row.get(10)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Update the status of every feedback row matching `uti` (that
    /// is not already in the target status). Returns the number of
    /// rows updated.
    pub fn update_feedback_status(&self, uti: &str, new_status: &str) -> Result<usize, StoreError> {
        let now = Utc::now().timestamp();
        let rows = self.conn.execute(
            "UPDATE feedbacks \
             SET status = ?1, status_set_at = ?2 \
             WHERE uti = ?3 AND status != ?1",
            params![new_status, now, uti],
        )?;
        Ok(rows)
    }

    /// Count of feedback rows grouped by `reason_code`, sorted
    /// descending by count. Used by rejection analytics to surface
    /// the most common TR rejection reasons.
    pub fn count_feedbacks_by_reason(
        &self,
        regime: Option<Regime>,
    ) -> Result<Vec<(String, i64)>, StoreError> {
        let mut sql = String::from(
            "SELECT COALESCE(reason_code, '(none)') AS rc, COUNT(*) AS n \
             FROM feedbacks WHERE 1=1",
        );
        let mut bind: Vec<Value> = Vec::new();
        if let Some(r) = regime {
            sql.push_str(" AND regime = ?");
            bind.push(Value::Text(
                match r {
                    Regime::Emir => "EMIR",
                    Regime::Sftr => "SFTR",
                }
                .to_owned(),
            ));
        }
        sql.push_str(" GROUP BY rc ORDER BY n DESC, rc ASC");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(bind), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Count of feedback rows grouped by `uti`, sorted descending.
    /// Used to identify UTIs that have been rejected repeatedly.
    pub fn count_feedbacks_by_uti(
        &self,
        regime: Option<Regime>,
    ) -> Result<Vec<(String, i64)>, StoreError> {
        let mut sql = String::from(
            "SELECT COALESCE(uti, '(none)') AS u, COUNT(*) AS n \
             FROM feedbacks WHERE 1=1",
        );
        let mut bind: Vec<Value> = Vec::new();
        if let Some(r) = regime {
            sql.push_str(" AND regime = ?");
            bind.push(Value::Text(
                match r {
                    Regime::Emir => "EMIR",
                    Regime::Sftr => "SFTR",
                }
                .to_owned(),
            ));
        }
        sql.push_str(" GROUP BY u ORDER BY n DESC, u ASC");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(bind), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
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
