//! Arrow `RecordBatch` → `Vec<EmirRecord>` / `Vec<SftrRecord>`
//! with a `canonical_field → user_column` mapping.
//!
//! **Type contract** (strict): when the user provides a mapping
//! entry `("uti", "MyUtiCol")`, the Arrow column `MyUtiCol` MUST
//! have the same Arrow type as the canonical EMIR / SFTR schema
//! emits via `opendqi_io::{emir_schema, sftr_schema}`. This is
//! by design — keeping Rust-side conversion strict avoids the
//! combinatorial cost of inferring types from arbitrary input.
//! Users with a "string everywhere" Arrow table cast their
//! columns in Python before calling `scan_table`:
//!
//! ```python
//! import pyarrow as pa
//! table = table.set_column(
//!     idx,
//!     "MaturityDate",
//!     pa.compute.cast(table.column("MaturityDate"), pa.date32()),
//! )
//! ```

use std::collections::HashMap;

use anyhow::{anyhow, Context, Result};
use arrow::array::{
    Array, BooleanArray, Date32Array, Decimal128Array, RecordBatch, StringArray,
    TimestampMicrosecondArray,
};
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use rust_decimal::Decimal;

use opendqi_core::{
    EmirRecord, FeedbackRecord, FeedbackType, MarginStateRecord, Regime, SftrRecord, TrStateRecord,
};

/// `canonical_field_name → user_column_name` mapping (same direction
/// as the CSV `mapping.fields` in `csv_in.rs:35-45`).
pub type Mapping = HashMap<String, String>;

// =================================================================
// Helpers — read one column-typed value at a row index
// =================================================================

fn col<'a>(batch: &'a RecordBatch, mapping: &Mapping, canonical: &str) -> Option<&'a dyn Array> {
    let user_col = mapping.get(canonical)?;
    batch.column_by_name(user_col).map(|c| c.as_ref())
}

fn pick_str(batch: &RecordBatch, mapping: &Mapping, canonical: &str, row: usize) -> Option<String> {
    let arr = col(batch, mapping, canonical)?;
    let s = arr.as_any().downcast_ref::<StringArray>()?;
    if s.is_null(row) {
        None
    } else {
        Some(s.value(row).to_string())
    }
}

fn pick_bool(batch: &RecordBatch, mapping: &Mapping, canonical: &str, row: usize) -> Option<bool> {
    let arr = col(batch, mapping, canonical)?;
    let b = arr.as_any().downcast_ref::<BooleanArray>()?;
    if b.is_null(row) {
        None
    } else {
        Some(b.value(row))
    }
}

fn pick_decimal(
    batch: &RecordBatch,
    mapping: &Mapping,
    canonical: &str,
    row: usize,
) -> Option<Decimal> {
    let arr = col(batch, mapping, canonical)?;
    let d = arr.as_any().downcast_ref::<Decimal128Array>()?;
    if d.is_null(row) {
        return None;
    }
    let raw = d.value(row);
    let scale = d.scale();
    // The canonical Arrow schema uses Decimal128(38, 10), so every
    // round-tripped value comes back with `scale == 10` regardless
    // of its original significant-digit count. `normalize()`
    // strips trailing zeros so e.g. `1_000_000.0000000000` reads
    // back as `1_000_000` — without this, the
    // `EMIR.VLD.NOTIONAL_PRECISION` and `_BY_CURRENCY` checks
    // would fire 1× per row simply because of the Arrow round-trip
    // padding, not because the actual value violated the ESMA
    // `decimal:18.5` precision.
    Decimal::try_from_i128_with_scale(raw, scale as u32)
        .ok()
        .map(|d| d.normalize())
}

fn pick_date(
    batch: &RecordBatch,
    mapping: &Mapping,
    canonical: &str,
    row: usize,
) -> Option<NaiveDate> {
    let arr = col(batch, mapping, canonical)?;
    let d = arr.as_any().downcast_ref::<Date32Array>()?;
    if d.is_null(row) {
        return None;
    }
    let days = d.value(row);
    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1)?;
    epoch.checked_add_signed(chrono::Duration::days(days as i64))
}

fn pick_timestamp(
    batch: &RecordBatch,
    mapping: &Mapping,
    canonical: &str,
    row: usize,
) -> Option<DateTime<Utc>> {
    let arr = col(batch, mapping, canonical)?;
    let t = arr.as_any().downcast_ref::<TimestampMicrosecondArray>()?;
    if t.is_null(row) {
        return None;
    }
    let micros = t.value(row);
    Utc.timestamp_micros(micros).single()
}

// =================================================================
// Public conversion entry points
// =================================================================

/// Project an Arrow `RecordBatch` into `Vec<EmirRecord>` using the
/// canonical-field-to-user-column mapping. Records not mapped
/// (or absent at runtime) are emitted with all fields `None` —
/// downstream DQ checks (`EMIR.COMP.*` family) surface the
/// missingness naturally.
///
/// Field names below MUST match the actual `EmirRecord` struct
/// in `crates/opendqi-core/src/model.rs` byte-for-byte.
pub fn batch_to_emir_records(batch: &RecordBatch, mapping: &Mapping) -> Result<Vec<EmirRecord>> {
    validate_mapping_columns(batch, mapping)?;
    let n = batch.num_rows();
    let mut out = Vec::with_capacity(n);
    for row in 0..n {
        out.push(EmirRecord {
            source_file: None,
            record_id: pick_str(batch, mapping, "record_id", row),
            uti: pick_str(batch, mapping, "uti", row),
            prior_uti: pick_str(batch, mapping, "prior_uti", row),
            action_type: pick_str(batch, mapping, "action_type", row),
            event_type: pick_str(batch, mapping, "event_type", row),
            entity_responsible_for_reporting: pick_str(
                batch,
                mapping,
                "entity_responsible_for_reporting",
                row,
            ),
            counterparty_1: pick_str(batch, mapping, "counterparty_1", row),
            counterparty_2: pick_str(batch, mapping, "counterparty_2", row),
            asset_class: pick_str(batch, mapping, "asset_class", row),
            product_id: pick_str(batch, mapping, "product_id", row),
            underlying_id: pick_str(batch, mapping, "underlying_id", row),
            notional_amount: pick_decimal(batch, mapping, "notional_amount", row),
            notional_currency: pick_str(batch, mapping, "notional_currency", row),
            price: pick_decimal(batch, mapping, "price", row),
            price_currency: pick_str(batch, mapping, "price_currency", row),
            execution_timestamp: pick_timestamp(batch, mapping, "execution_timestamp", row),
            event_timestamp: pick_timestamp(batch, mapping, "event_timestamp", row),
            reporting_timestamp: pick_timestamp(batch, mapping, "reporting_timestamp", row),
            effective_date: pick_date(batch, mapping, "effective_date", row),
            maturity_date: pick_date(batch, mapping, "maturity_date", row),
            termination_date: pick_date(batch, mapping, "termination_date", row),
            valuation_amount: pick_decimal(batch, mapping, "valuation_amount", row),
            valuation_currency: pick_str(batch, mapping, "valuation_currency", row),
            valuation_timestamp: pick_timestamp(batch, mapping, "valuation_timestamp", row),
            initial_margin_posted: pick_decimal(batch, mapping, "initial_margin_posted", row),
            initial_margin_collected: pick_decimal(batch, mapping, "initial_margin_collected", row),
            variation_margin_posted: pick_decimal(batch, mapping, "variation_margin_posted", row),
            variation_margin_collected: pick_decimal(
                batch,
                mapping,
                "variation_margin_collected",
                row,
            ),
            collateral_portfolio_code: pick_str(batch, mapping, "collateral_portfolio_code", row),
            clearing_status: pick_str(batch, mapping, "clearing_status", row),
            collateralisation_category: pick_str(batch, mapping, "collateralisation_category", row),
            leg2_notional_amount: pick_decimal(batch, mapping, "leg2_notional_amount", row),
            leg2_notional_currency: pick_str(batch, mapping, "leg2_notional_currency", row),
            leg1_payment_frequency: pick_str(batch, mapping, "leg1_payment_frequency", row),
            leg2_payment_frequency: pick_str(batch, mapping, "leg2_payment_frequency", row),
            clearing_ccp_lei: pick_str(batch, mapping, "clearing_ccp_lei", row),
            intragroup_indicator: pick_bool(batch, mapping, "intragroup_indicator", row),
            hedging_indicator: pick_bool(batch, mapping, "hedging_indicator", row),
            valuation_type: pick_str(batch, mapping, "valuation_type", row),
            trading_capacity: pick_str(batch, mapping, "trading_capacity", row),
            commercial_or_treasury_financing: pick_bool(
                batch,
                mapping,
                "commercial_or_treasury_financing",
                row,
            ),
            reporting_obligation_indicator: pick_str(
                batch,
                mapping,
                "reporting_obligation_indicator",
                row,
            ),
            corporate_sector: pick_str(batch, mapping, "corporate_sector", row),
            nature: pick_str(batch, mapping, "nature", row),
            master_agreement_type: pick_str(batch, mapping, "master_agreement_type", row),
            master_agreement_version: pick_str(batch, mapping, "master_agreement_version", row),
            confirmation_method: pick_str(batch, mapping, "confirmation_method", row),
            mtm_value_change: pick_decimal(batch, mapping, "mtm_value_change", row),
            delta: pick_decimal(batch, mapping, "delta", row),
            gamma: pick_decimal(batch, mapping, "gamma", row),
            vega: pick_decimal(batch, mapping, "vega", row),
            source_system: pick_str(batch, mapping, "source_system", row),
            raw_fields: Default::default(),
        });
    }
    Ok(out)
}

/// Project an Arrow `RecordBatch` into `Vec<SftrRecord>`.
pub fn batch_to_sftr_records(batch: &RecordBatch, mapping: &Mapping) -> Result<Vec<SftrRecord>> {
    validate_mapping_columns(batch, mapping)?;
    let n = batch.num_rows();
    let mut out = Vec::with_capacity(n);
    for row in 0..n {
        out.push(SftrRecord {
            source_file: None,
            record_id: pick_str(batch, mapping, "record_id", row),
            uti: pick_str(batch, mapping, "uti", row),
            prior_uti: pick_str(batch, mapping, "prior_uti", row),
            action_type: pick_str(batch, mapping, "action_type", row),
            event_type: pick_str(batch, mapping, "event_type", row),
            entity_responsible_for_reporting: pick_str(
                batch,
                mapping,
                "entity_responsible_for_reporting",
                row,
            ),
            counterparty_1: pick_str(batch, mapping, "counterparty_1", row),
            counterparty_2: pick_str(batch, mapping, "counterparty_2", row),
            sft_type: pick_str(batch, mapping, "sft_type", row),
            master_agreement_type: pick_str(batch, mapping, "master_agreement_type", row),
            master_agreement_version: pick_str(batch, mapping, "master_agreement_version", row),
            loan_value: pick_decimal(batch, mapping, "loan_value", row),
            loan_currency: pick_str(batch, mapping, "loan_currency", row),
            collateral_value: pick_decimal(batch, mapping, "collateral_value", row),
            collateral_currency: pick_str(batch, mapping, "collateral_currency", row),
            haircut: pick_decimal(batch, mapping, "haircut", row),
            reuse_indicator: pick_bool(batch, mapping, "reuse_indicator", row),
            rebate_rate: pick_decimal(batch, mapping, "rebate_rate", row),
            lending_fee: pick_decimal(batch, mapping, "lending_fee", row),
            execution_timestamp: pick_timestamp(batch, mapping, "execution_timestamp", row),
            event_timestamp: pick_timestamp(batch, mapping, "event_timestamp", row),
            reporting_timestamp: pick_timestamp(batch, mapping, "reporting_timestamp", row),
            effective_date: pick_date(batch, mapping, "effective_date", row),
            maturity_date: pick_date(batch, mapping, "maturity_date", row),
            termination_date: pick_date(batch, mapping, "termination_date", row),
            settlement_date: pick_date(batch, mapping, "settlement_date", row),
            collateral_portfolio_code: pick_str(batch, mapping, "collateral_portfolio_code", row),
            collateral_isin: pick_str(batch, mapping, "collateral_isin", row),
            security_identifier: pick_str(batch, mapping, "security_identifier", row),
            raw_fields: Default::default(),
        });
    }
    Ok(out)
}

/// Project an Arrow `RecordBatch` into `Vec<TrStateRecord>` for
/// the v0.15 Data Quality Pack Arrow inputs. Mirrors
/// [`batch_to_emir_records`] for the TSR (`auth.107`) layer —
/// minimal but correct field set (enough for the DQI computers
/// + the default_tr_state_checks granular checks ; specialised
/// fields like raw_fields stay empty).
pub fn batch_to_tr_state_records(
    batch: &RecordBatch,
    mapping: &Mapping,
) -> Result<Vec<TrStateRecord>> {
    validate_mapping_columns(batch, mapping)?;
    let n = batch.num_rows();
    let mut out = Vec::with_capacity(n);
    for row in 0..n {
        out.push(TrStateRecord {
            source_file: None,
            record_id: pick_str(batch, mapping, "record_id", row),
            regime: Regime::Emir,
            state_as_of: pick_timestamp(batch, mapping, "state_as_of", row),
            uti: pick_str(batch, mapping, "uti", row),
            reporting_counterparty: pick_str(batch, mapping, "reporting_counterparty", row),
            other_counterparty: pick_str(batch, mapping, "other_counterparty", row),
            status: pick_str(batch, mapping, "status", row),
            notional_amount: pick_decimal(batch, mapping, "notional_amount", row),
            notional_currency: pick_str(batch, mapping, "notional_currency", row),
            valuation_amount: pick_decimal(batch, mapping, "valuation_amount", row),
            valuation_currency: pick_str(batch, mapping, "valuation_currency", row),
            valuation_timestamp: pick_timestamp(batch, mapping, "valuation_timestamp", row),
            effective_date: pick_date(batch, mapping, "effective_date", row),
            maturity_date: pick_date(batch, mapping, "maturity_date", row),
            termination_date: pick_date(batch, mapping, "termination_date", row),
            collateral_portfolio_code: pick_str(batch, mapping, "collateral_portfolio_code", row),
            raw_fields: Default::default(),
        });
    }
    Ok(out)
}

/// Project an Arrow `RecordBatch` into `Vec<MarginStateRecord>`
/// for the v0.15 DQI pack — covers the MSR (`auth.109`) layer.
pub fn batch_to_margin_state_records(
    batch: &RecordBatch,
    mapping: &Mapping,
) -> Result<Vec<MarginStateRecord>> {
    validate_mapping_columns(batch, mapping)?;
    let n = batch.num_rows();
    let mut out = Vec::with_capacity(n);
    for row in 0..n {
        out.push(MarginStateRecord {
            source_file: None,
            record_id: pick_str(batch, mapping, "record_id", row),
            regime: Regime::Emir,
            uti: pick_str(batch, mapping, "uti", row),
            counterparty_1: pick_str(batch, mapping, "counterparty_1", row),
            counterparty_2: pick_str(batch, mapping, "counterparty_2", row),
            collateral_portfolio_code: pick_str(batch, mapping, "collateral_portfolio_code", row),
            initial_margin_posted_current: pick_decimal(
                batch,
                mapping,
                "initial_margin_posted_current",
                row,
            ),
            initial_margin_collected_current: pick_decimal(
                batch,
                mapping,
                "initial_margin_collected_current",
                row,
            ),
            variation_margin_posted_current: pick_decimal(
                batch,
                mapping,
                "variation_margin_posted_current",
                row,
            ),
            variation_margin_collected_current: pick_decimal(
                batch,
                mapping,
                "variation_margin_collected_current",
                row,
            ),
            margin_currency: pick_str(batch, mapping, "margin_currency", row),
            collateral_market_value: pick_decimal(batch, mapping, "collateral_market_value", row),
            haircut_applied: pick_decimal(batch, mapping, "haircut_applied", row),
            collateralization_category: pick_str(batch, mapping, "collateralization_category", row),
            state_as_of: pick_timestamp(batch, mapping, "state_as_of", row),
            raw_fields: Default::default(),
        });
    }
    Ok(out)
}

/// Project an Arrow `RecordBatch` into `Vec<FeedbackRecord>` for
/// the v0.15 DQI pack — covers the feedback (`auth.092`) layer.
///
/// `feedback_type` is a String column on the Arrow side. Values
/// MUST be one of `rejected` / `missing` / `inaccurate` /
/// `reconciliation_break` (case-insensitive). Unknown values fall
/// back to `Rejected` (the default) but emit no error — DQI
/// counters tolerate this.
pub fn batch_to_feedback_records(
    batch: &RecordBatch,
    mapping: &Mapping,
) -> Result<Vec<FeedbackRecord>> {
    validate_mapping_columns(batch, mapping)?;
    let n = batch.num_rows();
    let mut out = Vec::with_capacity(n);
    for row in 0..n {
        let feedback_type = pick_str(batch, mapping, "feedback_type", row)
            .map(|s| match s.trim().to_ascii_lowercase().as_str() {
                "rejected" => FeedbackType::Rejected,
                "missing" => FeedbackType::Missing,
                "inaccurate" => FeedbackType::Inaccurate,
                "reconciliation_break" => FeedbackType::ReconciliationBreak,
                _ => FeedbackType::Rejected,
            })
            .unwrap_or_default();
        out.push(FeedbackRecord {
            source_file: None,
            record_id: pick_str(batch, mapping, "record_id", row),
            regime: Regime::Emir,
            feedback_type,
            uti: pick_str(batch, mapping, "uti", row),
            reason_code: pick_str(batch, mapping, "reason_code", row),
            validation_rule_codes: Vec::new(),
            reason_description: pick_str(batch, mapping, "reason_description", row),
            reported_field: pick_str(batch, mapping, "reported_field", row),
            feedback_timestamp: pick_timestamp(batch, mapping, "feedback_timestamp", row),
        });
    }
    Ok(out)
}

/// Fail-early sanity check: every user-column the mapping points
/// at must exist on the batch. Catching this at the boundary
/// turns silent "all-None" records into a loud, actionable error.
fn validate_mapping_columns(batch: &RecordBatch, mapping: &Mapping) -> Result<()> {
    // `batch.schema()` returns an Arc — bind it to a local so the
    // `&str` borrows inside the HashSet outlive the iteration over
    // the mapping (E0716 otherwise).
    let schema = batch.schema();
    let schema_cols: std::collections::HashSet<&str> =
        schema.fields().iter().map(|f| f.name().as_str()).collect();
    for (canonical, user_col) in mapping {
        if !schema_cols.contains(user_col.as_str()) {
            return Err(anyhow!(
                "mapping entry {canonical:?} -> {user_col:?} refers to a column not present in the input table (columns: {schema_cols:?})"
            ))
            .with_context(|| format!("validating mapping for {canonical:?}"));
        }
    }
    Ok(())
}
