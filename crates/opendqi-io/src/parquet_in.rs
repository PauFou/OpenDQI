//! Read `EmirRecord` / `SftrRecord` slices back from a Parquet file
//! produced by [`crate::parquet_out`]. The inverse of the writer:
//! same schema contract (column names + datatypes), with three
//! tolerances:
//!
//! - **Missing columns** are accepted; the corresponding fields stay
//!   at `Default` / `None`. Forward-compat with v0 producers.
//! - **Extra unknown columns** are silently ignored.
//! - **`regime` validation**: if the file's `regime` column contains
//!   any row that does not match the regime the reader was asked for
//!   (e.g. `read_emir_parquet` invoked on an SFTR file), the call
//!   returns an error mentioning the mismatch.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use arrow_array::{
    Array, BooleanArray, Date32Array, Decimal128Array, RecordBatch, StringArray,
    TimestampMicrosecondArray,
};
use arrow_schema::Schema;
use chrono::{DateTime, Duration, NaiveDate, Utc};
use opendqi_core::{EmirRecord, SftrRecord};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use rust_decimal::Decimal;

use crate::parquet_out::DECIMAL_SCALE;

/// Read an EMIR Parquet file produced by
/// [`crate::write_emir_parquet`] (or any Parquet respecting the same
/// schema) into a vector of `EmirRecord`.
pub fn read_emir_parquet(path: &Path) -> Result<Vec<EmirRecord>> {
    let file =
        std::fs::File::open(path).with_context(|| format!("opening Parquet {}", path.display()))?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file)
        .with_context(|| format!("reading Parquet schema from {}", path.display()))?;
    let schema = builder.schema().clone();
    let reader = builder
        .build()
        .with_context(|| format!("building Parquet reader for {}", path.display()))?;

    let mut out: Vec<EmirRecord> = Vec::new();
    for batch in reader {
        let batch = batch.context("reading Parquet record batch")?;
        check_regime(&schema, &batch, "EMIR", path)?;
        decode_emir_batch(&schema, &batch, &mut out)?;
    }
    Ok(out)
}

/// Read an SFTR Parquet file produced by
/// [`crate::write_sftr_parquet`] into a vector of `SftrRecord`.
pub fn read_sftr_parquet(path: &Path) -> Result<Vec<SftrRecord>> {
    let file =
        std::fs::File::open(path).with_context(|| format!("opening Parquet {}", path.display()))?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file)
        .with_context(|| format!("reading Parquet schema from {}", path.display()))?;
    let schema = builder.schema().clone();
    let reader = builder
        .build()
        .with_context(|| format!("building Parquet reader for {}", path.display()))?;

    let mut out: Vec<SftrRecord> = Vec::new();
    for batch in reader {
        let batch = batch.context("reading Parquet record batch")?;
        check_regime(&schema, &batch, "SFTR", path)?;
        decode_sftr_batch(&schema, &batch, &mut out)?;
    }
    Ok(out)
}

// =================================================================
// Regime validation
// =================================================================

fn check_regime(schema: &Schema, batch: &RecordBatch, expected: &str, path: &Path) -> Result<()> {
    let Some(idx) = col_idx(schema, "regime") else {
        return Ok(()); // column absent → trust the caller
    };
    let arr = batch
        .column(idx)
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| anyhow!("regime column is not Utf8 in {}", path.display()))?;
    for row in 0..arr.len() {
        if arr.is_null(row) {
            continue;
        }
        let v = arr.value(row);
        if !v.eq_ignore_ascii_case(expected) {
            return Err(anyhow!(
                "Parquet {} contains a row with regime '{v}' but {expected} reader was invoked",
                path.display()
            ));
        }
    }
    Ok(())
}

// =================================================================
// EMIR decode
// =================================================================

fn decode_emir_batch(
    schema: &Schema,
    batch: &RecordBatch,
    out: &mut Vec<EmirRecord>,
) -> Result<()> {
    let source_file = col_idx(schema, "source_file");
    let record_id = col_idx(schema, "record_id");
    let uti = col_idx(schema, "uti");
    let prior_uti = col_idx(schema, "prior_uti");
    let action_type = col_idx(schema, "action_type");
    let event_type = col_idx(schema, "event_type");
    let err_lei = col_idx(schema, "entity_responsible_for_reporting");
    let cp1 = col_idx(schema, "counterparty_1");
    let cp2 = col_idx(schema, "counterparty_2");
    let asset_class = col_idx(schema, "asset_class");
    let product_id = col_idx(schema, "product_id");
    let underlying_id = col_idx(schema, "underlying_id");
    let notional_amount = col_idx(schema, "notional_amount");
    let notional_currency = col_idx(schema, "notional_currency");
    let price = col_idx(schema, "price");
    let price_currency = col_idx(schema, "price_currency");
    let execution_timestamp = col_idx(schema, "execution_timestamp");
    let event_timestamp = col_idx(schema, "event_timestamp");
    let reporting_timestamp = col_idx(schema, "reporting_timestamp");
    let effective_date = col_idx(schema, "effective_date");
    let maturity_date = col_idx(schema, "maturity_date");
    let termination_date = col_idx(schema, "termination_date");
    let valuation_amount = col_idx(schema, "valuation_amount");
    let valuation_currency = col_idx(schema, "valuation_currency");
    let valuation_timestamp = col_idx(schema, "valuation_timestamp");
    let im_posted = col_idx(schema, "initial_margin_posted");
    let im_collected = col_idx(schema, "initial_margin_collected");
    let vm_posted = col_idx(schema, "variation_margin_posted");
    let vm_collected = col_idx(schema, "variation_margin_collected");
    let portfolio_code = col_idx(schema, "collateral_portfolio_code");
    let clearing_status = col_idx(schema, "clearing_status");
    let coll_category = col_idx(schema, "collateralisation_category");
    let leg2_notional = col_idx(schema, "leg2_notional_amount");
    let leg2_ccy = col_idx(schema, "leg2_notional_currency");
    let leg1_freq = col_idx(schema, "leg1_payment_frequency");
    let leg2_freq = col_idx(schema, "leg2_payment_frequency");
    let ccp_lei = col_idx(schema, "clearing_ccp_lei");
    let intragroup = col_idx(schema, "intragroup_indicator");
    let hedging = col_idx(schema, "hedging_indicator");
    let valuation_type = col_idx(schema, "valuation_type");
    let trading_capacity = col_idx(schema, "trading_capacity");
    let commercial = col_idx(schema, "commercial_or_treasury_financing");
    let reporting_obligation = col_idx(schema, "reporting_obligation_indicator");
    let corporate_sector = col_idx(schema, "corporate_sector");
    let nature = col_idx(schema, "nature");
    let ma_type = col_idx(schema, "master_agreement_type");
    let ma_version = col_idx(schema, "master_agreement_version");
    let confirmation = col_idx(schema, "confirmation_method");
    let mtm_change = col_idx(schema, "mtm_value_change");
    let delta = col_idx(schema, "delta");
    let gamma = col_idx(schema, "gamma");
    let vega = col_idx(schema, "vega");
    let raw_fields = col_idx(schema, "raw_fields");

    out.reserve(batch.num_rows());
    for row in 0..batch.num_rows() {
        out.push(EmirRecord {
            source_file: read_opt_str(batch, source_file, row)?,
            record_id: read_opt_str(batch, record_id, row)?,
            uti: read_opt_str(batch, uti, row)?,
            prior_uti: read_opt_str(batch, prior_uti, row)?,
            action_type: read_opt_str(batch, action_type, row)?,
            event_type: read_opt_str(batch, event_type, row)?,
            entity_responsible_for_reporting: read_opt_str(batch, err_lei, row)?,
            counterparty_1: read_opt_str(batch, cp1, row)?,
            counterparty_2: read_opt_str(batch, cp2, row)?,
            asset_class: read_opt_str(batch, asset_class, row)?,
            product_id: read_opt_str(batch, product_id, row)?,
            underlying_id: read_opt_str(batch, underlying_id, row)?,
            notional_amount: read_opt_decimal(batch, notional_amount, row)?,
            notional_currency: read_opt_str(batch, notional_currency, row)?,
            price: read_opt_decimal(batch, price, row)?,
            price_currency: read_opt_str(batch, price_currency, row)?,
            execution_timestamp: read_opt_ts(batch, execution_timestamp, row)?,
            event_timestamp: read_opt_ts(batch, event_timestamp, row)?,
            reporting_timestamp: read_opt_ts(batch, reporting_timestamp, row)?,
            effective_date: read_opt_date(batch, effective_date, row)?,
            maturity_date: read_opt_date(batch, maturity_date, row)?,
            termination_date: read_opt_date(batch, termination_date, row)?,
            valuation_amount: read_opt_decimal(batch, valuation_amount, row)?,
            valuation_currency: read_opt_str(batch, valuation_currency, row)?,
            valuation_timestamp: read_opt_ts(batch, valuation_timestamp, row)?,
            initial_margin_posted: read_opt_decimal(batch, im_posted, row)?,
            initial_margin_collected: read_opt_decimal(batch, im_collected, row)?,
            variation_margin_posted: read_opt_decimal(batch, vm_posted, row)?,
            variation_margin_collected: read_opt_decimal(batch, vm_collected, row)?,
            collateral_portfolio_code: read_opt_str(batch, portfolio_code, row)?,
            clearing_status: read_opt_str(batch, clearing_status, row)?,
            collateralisation_category: read_opt_str(batch, coll_category, row)?,
            leg2_notional_amount: read_opt_decimal(batch, leg2_notional, row)?,
            leg2_notional_currency: read_opt_str(batch, leg2_ccy, row)?,
            leg1_payment_frequency: read_opt_str(batch, leg1_freq, row)?,
            leg2_payment_frequency: read_opt_str(batch, leg2_freq, row)?,
            clearing_ccp_lei: read_opt_str(batch, ccp_lei, row)?,
            intragroup_indicator: read_opt_bool(batch, intragroup, row)?,
            hedging_indicator: read_opt_bool(batch, hedging, row)?,
            valuation_type: read_opt_str(batch, valuation_type, row)?,
            trading_capacity: read_opt_str(batch, trading_capacity, row)?,
            commercial_or_treasury_financing: read_opt_bool(batch, commercial, row)?,
            reporting_obligation_indicator: read_opt_str(batch, reporting_obligation, row)?,
            corporate_sector: read_opt_str(batch, corporate_sector, row)?,
            nature: read_opt_str(batch, nature, row)?,
            master_agreement_type: read_opt_str(batch, ma_type, row)?,
            master_agreement_version: read_opt_str(batch, ma_version, row)?,
            confirmation_method: read_opt_str(batch, confirmation, row)?,
            mtm_value_change: read_opt_decimal(batch, mtm_change, row)?,
            delta: read_opt_decimal(batch, delta, row)?,
            gamma: read_opt_decimal(batch, gamma, row)?,
            vega: read_opt_decimal(batch, vega, row)?,
            raw_fields: read_raw_fields(batch, raw_fields, row)?,
        });
    }
    Ok(())
}

// =================================================================
// SFTR decode
// =================================================================

fn decode_sftr_batch(
    schema: &Schema,
    batch: &RecordBatch,
    out: &mut Vec<SftrRecord>,
) -> Result<()> {
    let source_file = col_idx(schema, "source_file");
    let record_id = col_idx(schema, "record_id");
    let uti = col_idx(schema, "uti");
    let prior_uti = col_idx(schema, "prior_uti");
    let action_type = col_idx(schema, "action_type");
    let event_type = col_idx(schema, "event_type");
    let err_lei = col_idx(schema, "entity_responsible_for_reporting");
    let cp1 = col_idx(schema, "counterparty_1");
    let cp2 = col_idx(schema, "counterparty_2");
    let sft_type = col_idx(schema, "sft_type");
    let ma_type = col_idx(schema, "master_agreement_type");
    let ma_version = col_idx(schema, "master_agreement_version");
    let loan_value = col_idx(schema, "loan_value");
    let loan_currency = col_idx(schema, "loan_currency");
    let collateral_value = col_idx(schema, "collateral_value");
    let collateral_currency = col_idx(schema, "collateral_currency");
    let haircut = col_idx(schema, "haircut");
    let reuse_indicator = col_idx(schema, "reuse_indicator");
    let rebate_rate = col_idx(schema, "rebate_rate");
    let lending_fee = col_idx(schema, "lending_fee");
    let execution_timestamp = col_idx(schema, "execution_timestamp");
    let event_timestamp = col_idx(schema, "event_timestamp");
    let reporting_timestamp = col_idx(schema, "reporting_timestamp");
    let effective_date = col_idx(schema, "effective_date");
    let maturity_date = col_idx(schema, "maturity_date");
    let termination_date = col_idx(schema, "termination_date");
    let settlement_date = col_idx(schema, "settlement_date");
    let portfolio_code = col_idx(schema, "collateral_portfolio_code");
    let collateral_isin = col_idx(schema, "collateral_isin");
    let raw_fields = col_idx(schema, "raw_fields");

    out.reserve(batch.num_rows());
    for row in 0..batch.num_rows() {
        out.push(SftrRecord {
            source_file: read_opt_str(batch, source_file, row)?,
            record_id: read_opt_str(batch, record_id, row)?,
            uti: read_opt_str(batch, uti, row)?,
            prior_uti: read_opt_str(batch, prior_uti, row)?,
            action_type: read_opt_str(batch, action_type, row)?,
            event_type: read_opt_str(batch, event_type, row)?,
            entity_responsible_for_reporting: read_opt_str(batch, err_lei, row)?,
            counterparty_1: read_opt_str(batch, cp1, row)?,
            counterparty_2: read_opt_str(batch, cp2, row)?,
            sft_type: read_opt_str(batch, sft_type, row)?,
            master_agreement_type: read_opt_str(batch, ma_type, row)?,
            master_agreement_version: read_opt_str(batch, ma_version, row)?,
            loan_value: read_opt_decimal(batch, loan_value, row)?,
            loan_currency: read_opt_str(batch, loan_currency, row)?,
            collateral_value: read_opt_decimal(batch, collateral_value, row)?,
            collateral_currency: read_opt_str(batch, collateral_currency, row)?,
            haircut: read_opt_decimal(batch, haircut, row)?,
            reuse_indicator: read_opt_bool(batch, reuse_indicator, row)?,
            rebate_rate: read_opt_decimal(batch, rebate_rate, row)?,
            lending_fee: read_opt_decimal(batch, lending_fee, row)?,
            execution_timestamp: read_opt_ts(batch, execution_timestamp, row)?,
            event_timestamp: read_opt_ts(batch, event_timestamp, row)?,
            reporting_timestamp: read_opt_ts(batch, reporting_timestamp, row)?,
            effective_date: read_opt_date(batch, effective_date, row)?,
            maturity_date: read_opt_date(batch, maturity_date, row)?,
            termination_date: read_opt_date(batch, termination_date, row)?,
            settlement_date: read_opt_date(batch, settlement_date, row)?,
            collateral_portfolio_code: read_opt_str(batch, portfolio_code, row)?,
            collateral_isin: read_opt_str(batch, collateral_isin, row)?,
            raw_fields: read_raw_fields(batch, raw_fields, row)?,
        });
    }
    Ok(())
}

// =================================================================
// Column readers
// =================================================================

fn col_idx(schema: &Schema, name: &str) -> Option<usize> {
    schema.index_of(name).ok()
}

fn read_opt_str(batch: &RecordBatch, idx: Option<usize>, row: usize) -> Result<Option<String>> {
    let Some(idx) = idx else {
        return Ok(None);
    };
    let arr = batch
        .column(idx)
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| anyhow!("column at index {idx} is not Utf8"))?;
    if arr.is_null(row) {
        return Ok(None);
    }
    Ok(Some(arr.value(row).to_owned()))
}

fn read_opt_bool(batch: &RecordBatch, idx: Option<usize>, row: usize) -> Result<Option<bool>> {
    let Some(idx) = idx else {
        return Ok(None);
    };
    let arr = batch
        .column(idx)
        .as_any()
        .downcast_ref::<BooleanArray>()
        .ok_or_else(|| anyhow!("column at index {idx} is not Boolean"))?;
    if arr.is_null(row) {
        return Ok(None);
    }
    Ok(Some(arr.value(row)))
}

fn read_opt_ts(
    batch: &RecordBatch,
    idx: Option<usize>,
    row: usize,
) -> Result<Option<DateTime<Utc>>> {
    let Some(idx) = idx else {
        return Ok(None);
    };
    let arr = batch
        .column(idx)
        .as_any()
        .downcast_ref::<TimestampMicrosecondArray>()
        .ok_or_else(|| anyhow!("column at index {idx} is not TimestampMicrosecond"))?;
    if arr.is_null(row) {
        return Ok(None);
    }
    let micros = arr.value(row);
    Ok(DateTime::<Utc>::from_timestamp_micros(micros))
}

fn read_opt_date(batch: &RecordBatch, idx: Option<usize>, row: usize) -> Result<Option<NaiveDate>> {
    let Some(idx) = idx else {
        return Ok(None);
    };
    let arr = batch
        .column(idx)
        .as_any()
        .downcast_ref::<Date32Array>()
        .ok_or_else(|| anyhow!("column at index {idx} is not Date32"))?;
    if arr.is_null(row) {
        return Ok(None);
    }
    Ok(days_to_date(arr.value(row)))
}

fn read_opt_decimal(
    batch: &RecordBatch,
    idx: Option<usize>,
    row: usize,
) -> Result<Option<Decimal>> {
    let Some(idx) = idx else {
        return Ok(None);
    };
    let arr = batch
        .column(idx)
        .as_any()
        .downcast_ref::<Decimal128Array>()
        .ok_or_else(|| anyhow!("column at index {idx} is not Decimal128"))?;
    if arr.is_null(row) {
        return Ok(None);
    }
    Ok(Some(i128_to_decimal(arr.value(row), DECIMAL_SCALE as u32)))
}

fn read_raw_fields(
    batch: &RecordBatch,
    idx: Option<usize>,
    row: usize,
) -> Result<BTreeMap<String, String>> {
    let Some(json) = read_opt_str(batch, idx, row)? else {
        return Ok(BTreeMap::new());
    };
    Ok(json_to_raw_fields(&json))
}

// =================================================================
// Inverse conversions
// =================================================================

fn i128_to_decimal(mantissa: i128, scale: u32) -> Decimal {
    // Build "<mantissa> / 10^scale" without going through f64.
    // Decimal::from_i128_with_scale exists in rust_decimal ≥ 1.27 and
    // gives an exact value round-trip with `decimal_to_i128` in
    // parquet_out. `.normalize()` strips trailing zeros so the
    // recovered Decimal carries the *minimal* scale — important for
    // checks that gate on `Decimal::scale()` (e.g.
    // `EMIR.VLD.NOTIONAL_PRECISION_BY_CURRENCY`).
    Decimal::from_i128_with_scale(mantissa, scale).normalize()
}

fn days_to_date(days: i32) -> Option<NaiveDate> {
    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1)?;
    epoch.checked_add_signed(Duration::days(days as i64))
}

fn json_to_raw_fields(s: &str) -> BTreeMap<String, String> {
    // Best-effort: a malformed raw_fields cell yields an empty map
    // rather than failing the whole read.
    serde_json::from_str::<BTreeMap<String, String>>(s).unwrap_or_default()
}

// Re-export for tests
#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal as D;
    use std::str::FromStr;

    #[test]
    fn i128_to_decimal_positive_roundtrip() {
        // 1234.56789 with scale 10 → mantissa 12_345_678_900_000. The
        // recovered Decimal is normalised (trailing zeros stripped),
        // so it compares equal to "1234.56789" (scale 5) — the value
        // is preserved exactly, the scale is the minimal one.
        let d = i128_to_decimal(12_345_678_900_000_i128, 10);
        assert_eq!(d, D::from_str("1234.56789").unwrap());
    }

    #[test]
    fn i128_to_decimal_negative_roundtrip() {
        let d = i128_to_decimal(-10_000_000_000_i128, 10);
        assert_eq!(d, D::from_str("-1").unwrap());
    }

    #[test]
    fn days_to_date_roundtrip() {
        let d = days_to_date(20_587).unwrap(); // 1970-01-01 + 20587d ≈ 2026-05-14
        assert_eq!(d, NaiveDate::from_ymd_opt(2026, 5, 14).unwrap());
    }

    #[test]
    fn json_to_raw_fields_recovers_map() {
        let m = json_to_raw_fields(r#"{"baz":"qux","foo":"bar"}"#);
        assert_eq!(m.get("foo").map(String::as_str), Some("bar"));
        assert_eq!(m.get("baz").map(String::as_str), Some("qux"));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn json_to_raw_fields_malformed_yields_empty() {
        let m = json_to_raw_fields("not-json");
        assert!(m.is_empty());
    }
}
