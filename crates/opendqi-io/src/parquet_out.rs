//! Write `EmirRecord` / `SftrRecord` slices to a Parquet file with a
//! stable, analytics-friendly schema (Snappy-compressed, Apache Arrow
//! 53.x). See `docs/parquet-normalize.md` for the schema reference and
//! downstream usage (DuckDB / Polars / PyArrow).
//!
//! Decimal columns use `Decimal128(38, 10)` — 28 integer digits + 10
//! fractional digits cover regulatory notionals up to 10^28 and
//! haircuts at 1e-10 precision. Timestamps use microsecond resolution
//! in UTC. Dates use Arrow's `Date32` (days since 1970-01-01). The
//! catch-all `raw_fields` map is serialised as a JSON object string.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use arrow_array::builder::{
    BooleanBuilder, Date32Builder, Decimal128Builder, StringBuilder, TimestampMicrosecondBuilder,
};
use arrow_array::{ArrayRef, RecordBatch};
use arrow_schema::{DataType, Field, Schema, SchemaRef, TimeUnit};
use chrono::{DateTime, NaiveDate, Utc};
use opendqi_core::{EmirRecord, SftrRecord};
use parquet::arrow::ArrowWriter;
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;
use rust_decimal::Decimal;

pub(crate) const DECIMAL_PRECISION: u8 = 38;
pub(crate) const DECIMAL_SCALE: i8 = 10;
pub(crate) const TS_UTC: &str = "UTC";

// =================================================================
// Public API
// =================================================================

/// Write a slice of `EmirRecord` to a Parquet file with the canonical
/// EMIR schema. Compressed with Snappy.
pub fn write_emir_parquet(path: &Path, records: &[EmirRecord]) -> Result<()> {
    let schema = emir_schema();
    let batch = build_emir_batch(&schema, records)?;
    write_batch(path, schema, batch)
}

/// Write a slice of `SftrRecord` to a Parquet file with the canonical
/// SFTR schema. Compressed with Snappy.
pub fn write_sftr_parquet(path: &Path, records: &[SftrRecord]) -> Result<()> {
    let schema = sftr_schema();
    let batch = build_sftr_batch(&schema, records)?;
    write_batch(path, schema, batch)
}

fn write_batch(path: &Path, schema: SchemaRef, batch: RecordBatch) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating parent directory {}", parent.display()))?;
        }
    }
    let file = std::fs::File::create(path)
        .with_context(|| format!("creating parquet file {}", path.display()))?;
    let props = WriterProperties::builder()
        .set_compression(Compression::SNAPPY)
        .build();
    let mut writer = ArrowWriter::try_new(file, schema, Some(props))
        .context("constructing Parquet ArrowWriter")?;
    writer
        .write(&batch)
        .context("writing Parquet record batch")?;
    writer.close().context("closing Parquet writer")?;
    Ok(())
}

// =================================================================
// EMIR schema + batch
// =================================================================

fn emir_schema() -> SchemaRef {
    Arc::new(Schema::new(emir_fields()))
}

fn emir_fields() -> Vec<Field> {
    let s = utf8;
    let d = decimal;
    let ts = ts_micros;
    let dt = date32;
    let b = boolean;
    vec![
        s("source_file"),
        s("record_id"),
        s("regime"),
        s("uti"),
        s("prior_uti"),
        s("action_type"),
        s("event_type"),
        s("entity_responsible_for_reporting"),
        s("counterparty_1"),
        s("counterparty_2"),
        s("asset_class"),
        s("product_id"),
        s("underlying_id"),
        d("notional_amount"),
        s("notional_currency"),
        d("price"),
        s("price_currency"),
        ts("execution_timestamp"),
        ts("event_timestamp"),
        ts("reporting_timestamp"),
        dt("effective_date"),
        dt("maturity_date"),
        dt("termination_date"),
        d("valuation_amount"),
        s("valuation_currency"),
        ts("valuation_timestamp"),
        d("initial_margin_posted"),
        d("initial_margin_collected"),
        d("variation_margin_posted"),
        d("variation_margin_collected"),
        s("collateral_portfolio_code"),
        s("clearing_status"),
        s("collateralisation_category"),
        d("leg2_notional_amount"),
        s("leg2_notional_currency"),
        s("leg1_payment_frequency"),
        s("leg2_payment_frequency"),
        s("clearing_ccp_lei"),
        b("intragroup_indicator"),
        b("hedging_indicator"),
        s("valuation_type"),
        s("trading_capacity"),
        b("commercial_or_treasury_financing"),
        s("reporting_obligation_indicator"),
        s("corporate_sector"),
        s("nature"),
        s("master_agreement_type"),
        s("master_agreement_version"),
        s("confirmation_method"),
        d("mtm_value_change"),
        d("delta"),
        d("gamma"),
        d("vega"),
        s("raw_fields"),
    ]
}

fn build_emir_batch(schema: &SchemaRef, records: &[EmirRecord]) -> Result<RecordBatch> {
    let n = records.len();
    let mut source_file = StringBuilder::new();
    let mut record_id = StringBuilder::new();
    let mut regime = StringBuilder::new();
    let mut uti = StringBuilder::new();
    let mut prior_uti = StringBuilder::new();
    let mut action_type = StringBuilder::new();
    let mut event_type = StringBuilder::new();
    let mut entity_responsible_for_reporting = StringBuilder::new();
    let mut counterparty_1 = StringBuilder::new();
    let mut counterparty_2 = StringBuilder::new();
    let mut asset_class = StringBuilder::new();
    let mut product_id = StringBuilder::new();
    let mut underlying_id = StringBuilder::new();
    let mut notional_amount = decimal_builder();
    let mut notional_currency = StringBuilder::new();
    let mut price = decimal_builder();
    let mut price_currency = StringBuilder::new();
    let mut execution_timestamp = ts_builder();
    let mut event_timestamp = ts_builder();
    let mut reporting_timestamp = ts_builder();
    let mut effective_date = Date32Builder::with_capacity(n);
    let mut maturity_date = Date32Builder::with_capacity(n);
    let mut termination_date = Date32Builder::with_capacity(n);
    let mut valuation_amount = decimal_builder();
    let mut valuation_currency = StringBuilder::new();
    let mut valuation_timestamp = ts_builder();
    let mut initial_margin_posted = decimal_builder();
    let mut initial_margin_collected = decimal_builder();
    let mut variation_margin_posted = decimal_builder();
    let mut variation_margin_collected = decimal_builder();
    let mut collateral_portfolio_code = StringBuilder::new();
    let mut clearing_status = StringBuilder::new();
    let mut collateralisation_category = StringBuilder::new();
    let mut leg2_notional_amount = decimal_builder();
    let mut leg2_notional_currency = StringBuilder::new();
    let mut leg1_payment_frequency = StringBuilder::new();
    let mut leg2_payment_frequency = StringBuilder::new();
    let mut clearing_ccp_lei = StringBuilder::new();
    let mut intragroup_indicator = BooleanBuilder::with_capacity(n);
    let mut hedging_indicator = BooleanBuilder::with_capacity(n);
    let mut valuation_type = StringBuilder::new();
    let mut trading_capacity = StringBuilder::new();
    let mut commercial_or_treasury_financing = BooleanBuilder::with_capacity(n);
    let mut reporting_obligation_indicator = StringBuilder::new();
    let mut corporate_sector = StringBuilder::new();
    let mut nature = StringBuilder::new();
    let mut master_agreement_type = StringBuilder::new();
    let mut master_agreement_version = StringBuilder::new();
    let mut confirmation_method = StringBuilder::new();
    let mut mtm_value_change = decimal_builder();
    let mut delta = decimal_builder();
    let mut gamma = decimal_builder();
    let mut vega = decimal_builder();
    let mut raw_fields = StringBuilder::new();

    for r in records {
        push_opt_str(&mut source_file, r.source_file.as_deref());
        push_opt_str(&mut record_id, r.record_id.as_deref());
        push_opt_str(&mut regime, Some("EMIR"));
        push_opt_str(&mut uti, r.uti.as_deref());
        push_opt_str(&mut prior_uti, r.prior_uti.as_deref());
        push_opt_str(&mut action_type, r.action_type.as_deref());
        push_opt_str(&mut event_type, r.event_type.as_deref());
        push_opt_str(
            &mut entity_responsible_for_reporting,
            r.entity_responsible_for_reporting.as_deref(),
        );
        push_opt_str(&mut counterparty_1, r.counterparty_1.as_deref());
        push_opt_str(&mut counterparty_2, r.counterparty_2.as_deref());
        push_opt_str(&mut asset_class, r.asset_class.as_deref());
        push_opt_str(&mut product_id, r.product_id.as_deref());
        push_opt_str(&mut underlying_id, r.underlying_id.as_deref());
        push_opt_decimal(&mut notional_amount, r.notional_amount)?;
        push_opt_str(&mut notional_currency, r.notional_currency.as_deref());
        push_opt_decimal(&mut price, r.price)?;
        push_opt_str(&mut price_currency, r.price_currency.as_deref());
        push_opt_ts(&mut execution_timestamp, r.execution_timestamp);
        push_opt_ts(&mut event_timestamp, r.event_timestamp);
        push_opt_ts(&mut reporting_timestamp, r.reporting_timestamp);
        push_opt_date(&mut effective_date, r.effective_date);
        push_opt_date(&mut maturity_date, r.maturity_date);
        push_opt_date(&mut termination_date, r.termination_date);
        push_opt_decimal(&mut valuation_amount, r.valuation_amount)?;
        push_opt_str(&mut valuation_currency, r.valuation_currency.as_deref());
        push_opt_ts(&mut valuation_timestamp, r.valuation_timestamp);
        push_opt_decimal(&mut initial_margin_posted, r.initial_margin_posted)?;
        push_opt_decimal(&mut initial_margin_collected, r.initial_margin_collected)?;
        push_opt_decimal(&mut variation_margin_posted, r.variation_margin_posted)?;
        push_opt_decimal(
            &mut variation_margin_collected,
            r.variation_margin_collected,
        )?;
        push_opt_str(
            &mut collateral_portfolio_code,
            r.collateral_portfolio_code.as_deref(),
        );
        push_opt_str(&mut clearing_status, r.clearing_status.as_deref());
        push_opt_str(
            &mut collateralisation_category,
            r.collateralisation_category.as_deref(),
        );
        push_opt_decimal(&mut leg2_notional_amount, r.leg2_notional_amount)?;
        push_opt_str(
            &mut leg2_notional_currency,
            r.leg2_notional_currency.as_deref(),
        );
        push_opt_str(
            &mut leg1_payment_frequency,
            r.leg1_payment_frequency.as_deref(),
        );
        push_opt_str(
            &mut leg2_payment_frequency,
            r.leg2_payment_frequency.as_deref(),
        );
        push_opt_str(&mut clearing_ccp_lei, r.clearing_ccp_lei.as_deref());
        push_opt_bool(&mut intragroup_indicator, r.intragroup_indicator);
        push_opt_bool(&mut hedging_indicator, r.hedging_indicator);
        push_opt_str(&mut valuation_type, r.valuation_type.as_deref());
        push_opt_str(&mut trading_capacity, r.trading_capacity.as_deref());
        push_opt_bool(
            &mut commercial_or_treasury_financing,
            r.commercial_or_treasury_financing,
        );
        push_opt_str(
            &mut reporting_obligation_indicator,
            r.reporting_obligation_indicator.as_deref(),
        );
        push_opt_str(&mut corporate_sector, r.corporate_sector.as_deref());
        push_opt_str(&mut nature, r.nature.as_deref());
        push_opt_str(
            &mut master_agreement_type,
            r.master_agreement_type.as_deref(),
        );
        push_opt_str(
            &mut master_agreement_version,
            r.master_agreement_version.as_deref(),
        );
        push_opt_str(&mut confirmation_method, r.confirmation_method.as_deref());
        push_opt_decimal(&mut mtm_value_change, r.mtm_value_change)?;
        push_opt_decimal(&mut delta, r.delta)?;
        push_opt_decimal(&mut gamma, r.gamma)?;
        push_opt_decimal(&mut vega, r.vega)?;
        push_raw_fields(&mut raw_fields, &r.raw_fields);
    }

    let arrays: Vec<ArrayRef> = vec![
        Arc::new(source_file.finish()),
        Arc::new(record_id.finish()),
        Arc::new(regime.finish()),
        Arc::new(uti.finish()),
        Arc::new(prior_uti.finish()),
        Arc::new(action_type.finish()),
        Arc::new(event_type.finish()),
        Arc::new(entity_responsible_for_reporting.finish()),
        Arc::new(counterparty_1.finish()),
        Arc::new(counterparty_2.finish()),
        Arc::new(asset_class.finish()),
        Arc::new(product_id.finish()),
        Arc::new(underlying_id.finish()),
        Arc::new(finish_decimal(notional_amount)?),
        Arc::new(notional_currency.finish()),
        Arc::new(finish_decimal(price)?),
        Arc::new(price_currency.finish()),
        Arc::new(execution_timestamp.finish().with_timezone(TS_UTC)),
        Arc::new(event_timestamp.finish().with_timezone(TS_UTC)),
        Arc::new(reporting_timestamp.finish().with_timezone(TS_UTC)),
        Arc::new(effective_date.finish()),
        Arc::new(maturity_date.finish()),
        Arc::new(termination_date.finish()),
        Arc::new(finish_decimal(valuation_amount)?),
        Arc::new(valuation_currency.finish()),
        Arc::new(valuation_timestamp.finish().with_timezone(TS_UTC)),
        Arc::new(finish_decimal(initial_margin_posted)?),
        Arc::new(finish_decimal(initial_margin_collected)?),
        Arc::new(finish_decimal(variation_margin_posted)?),
        Arc::new(finish_decimal(variation_margin_collected)?),
        Arc::new(collateral_portfolio_code.finish()),
        Arc::new(clearing_status.finish()),
        Arc::new(collateralisation_category.finish()),
        Arc::new(finish_decimal(leg2_notional_amount)?),
        Arc::new(leg2_notional_currency.finish()),
        Arc::new(leg1_payment_frequency.finish()),
        Arc::new(leg2_payment_frequency.finish()),
        Arc::new(clearing_ccp_lei.finish()),
        Arc::new(intragroup_indicator.finish()),
        Arc::new(hedging_indicator.finish()),
        Arc::new(valuation_type.finish()),
        Arc::new(trading_capacity.finish()),
        Arc::new(commercial_or_treasury_financing.finish()),
        Arc::new(reporting_obligation_indicator.finish()),
        Arc::new(corporate_sector.finish()),
        Arc::new(nature.finish()),
        Arc::new(master_agreement_type.finish()),
        Arc::new(master_agreement_version.finish()),
        Arc::new(confirmation_method.finish()),
        Arc::new(finish_decimal(mtm_value_change)?),
        Arc::new(finish_decimal(delta)?),
        Arc::new(finish_decimal(gamma)?),
        Arc::new(finish_decimal(vega)?),
        Arc::new(raw_fields.finish()),
    ];
    RecordBatch::try_new(schema.clone(), arrays).context("constructing EMIR RecordBatch")
}

// =================================================================
// SFTR schema + batch
// =================================================================

fn sftr_schema() -> SchemaRef {
    Arc::new(Schema::new(sftr_fields()))
}

fn sftr_fields() -> Vec<Field> {
    let s = utf8;
    let d = decimal;
    let ts = ts_micros;
    let dt = date32;
    let b = boolean;
    vec![
        s("source_file"),
        s("record_id"),
        s("regime"),
        s("uti"),
        s("prior_uti"),
        s("action_type"),
        s("event_type"),
        s("entity_responsible_for_reporting"),
        s("counterparty_1"),
        s("counterparty_2"),
        s("sft_type"),
        s("master_agreement_type"),
        s("master_agreement_version"),
        d("loan_value"),
        s("loan_currency"),
        d("collateral_value"),
        s("collateral_currency"),
        d("haircut"),
        b("reuse_indicator"),
        d("rebate_rate"),
        d("lending_fee"),
        ts("execution_timestamp"),
        ts("event_timestamp"),
        ts("reporting_timestamp"),
        dt("effective_date"),
        dt("maturity_date"),
        dt("termination_date"),
        dt("settlement_date"),
        s("collateral_portfolio_code"),
        s("collateral_isin"),
        s("raw_fields"),
    ]
}

fn build_sftr_batch(schema: &SchemaRef, records: &[SftrRecord]) -> Result<RecordBatch> {
    let n = records.len();
    let mut source_file = StringBuilder::new();
    let mut record_id = StringBuilder::new();
    let mut regime = StringBuilder::new();
    let mut uti = StringBuilder::new();
    let mut prior_uti = StringBuilder::new();
    let mut action_type = StringBuilder::new();
    let mut event_type = StringBuilder::new();
    let mut entity_responsible_for_reporting = StringBuilder::new();
    let mut counterparty_1 = StringBuilder::new();
    let mut counterparty_2 = StringBuilder::new();
    let mut sft_type = StringBuilder::new();
    let mut master_agreement_type = StringBuilder::new();
    let mut master_agreement_version = StringBuilder::new();
    let mut loan_value = decimal_builder();
    let mut loan_currency = StringBuilder::new();
    let mut collateral_value = decimal_builder();
    let mut collateral_currency = StringBuilder::new();
    let mut haircut = decimal_builder();
    let mut reuse_indicator = BooleanBuilder::with_capacity(n);
    let mut rebate_rate = decimal_builder();
    let mut lending_fee = decimal_builder();
    let mut execution_timestamp = ts_builder();
    let mut event_timestamp = ts_builder();
    let mut reporting_timestamp = ts_builder();
    let mut effective_date = Date32Builder::with_capacity(n);
    let mut maturity_date = Date32Builder::with_capacity(n);
    let mut termination_date = Date32Builder::with_capacity(n);
    let mut settlement_date = Date32Builder::with_capacity(n);
    let mut collateral_portfolio_code = StringBuilder::new();
    let mut collateral_isin = StringBuilder::new();
    let mut raw_fields = StringBuilder::new();

    for r in records {
        push_opt_str(&mut source_file, r.source_file.as_deref());
        push_opt_str(&mut record_id, r.record_id.as_deref());
        push_opt_str(&mut regime, Some("SFTR"));
        push_opt_str(&mut uti, r.uti.as_deref());
        push_opt_str(&mut prior_uti, r.prior_uti.as_deref());
        push_opt_str(&mut action_type, r.action_type.as_deref());
        push_opt_str(&mut event_type, r.event_type.as_deref());
        push_opt_str(
            &mut entity_responsible_for_reporting,
            r.entity_responsible_for_reporting.as_deref(),
        );
        push_opt_str(&mut counterparty_1, r.counterparty_1.as_deref());
        push_opt_str(&mut counterparty_2, r.counterparty_2.as_deref());
        push_opt_str(&mut sft_type, r.sft_type.as_deref());
        push_opt_str(
            &mut master_agreement_type,
            r.master_agreement_type.as_deref(),
        );
        push_opt_str(
            &mut master_agreement_version,
            r.master_agreement_version.as_deref(),
        );
        push_opt_decimal(&mut loan_value, r.loan_value)?;
        push_opt_str(&mut loan_currency, r.loan_currency.as_deref());
        push_opt_decimal(&mut collateral_value, r.collateral_value)?;
        push_opt_str(&mut collateral_currency, r.collateral_currency.as_deref());
        push_opt_decimal(&mut haircut, r.haircut)?;
        push_opt_bool(&mut reuse_indicator, r.reuse_indicator);
        push_opt_decimal(&mut rebate_rate, r.rebate_rate)?;
        push_opt_decimal(&mut lending_fee, r.lending_fee)?;
        push_opt_ts(&mut execution_timestamp, r.execution_timestamp);
        push_opt_ts(&mut event_timestamp, r.event_timestamp);
        push_opt_ts(&mut reporting_timestamp, r.reporting_timestamp);
        push_opt_date(&mut effective_date, r.effective_date);
        push_opt_date(&mut maturity_date, r.maturity_date);
        push_opt_date(&mut termination_date, r.termination_date);
        push_opt_date(&mut settlement_date, r.settlement_date);
        push_opt_str(
            &mut collateral_portfolio_code,
            r.collateral_portfolio_code.as_deref(),
        );
        push_opt_str(&mut collateral_isin, r.collateral_isin.as_deref());
        push_raw_fields(&mut raw_fields, &r.raw_fields);
    }

    let arrays: Vec<ArrayRef> = vec![
        Arc::new(source_file.finish()),
        Arc::new(record_id.finish()),
        Arc::new(regime.finish()),
        Arc::new(uti.finish()),
        Arc::new(prior_uti.finish()),
        Arc::new(action_type.finish()),
        Arc::new(event_type.finish()),
        Arc::new(entity_responsible_for_reporting.finish()),
        Arc::new(counterparty_1.finish()),
        Arc::new(counterparty_2.finish()),
        Arc::new(sft_type.finish()),
        Arc::new(master_agreement_type.finish()),
        Arc::new(master_agreement_version.finish()),
        Arc::new(finish_decimal(loan_value)?),
        Arc::new(loan_currency.finish()),
        Arc::new(finish_decimal(collateral_value)?),
        Arc::new(collateral_currency.finish()),
        Arc::new(finish_decimal(haircut)?),
        Arc::new(reuse_indicator.finish()),
        Arc::new(finish_decimal(rebate_rate)?),
        Arc::new(finish_decimal(lending_fee)?),
        Arc::new(execution_timestamp.finish().with_timezone(TS_UTC)),
        Arc::new(event_timestamp.finish().with_timezone(TS_UTC)),
        Arc::new(reporting_timestamp.finish().with_timezone(TS_UTC)),
        Arc::new(effective_date.finish()),
        Arc::new(maturity_date.finish()),
        Arc::new(termination_date.finish()),
        Arc::new(settlement_date.finish()),
        Arc::new(collateral_portfolio_code.finish()),
        Arc::new(collateral_isin.finish()),
        Arc::new(raw_fields.finish()),
    ];
    RecordBatch::try_new(schema.clone(), arrays).context("constructing SFTR RecordBatch")
}

// =================================================================
// Field constructors
// =================================================================

fn utf8(name: &str) -> Field {
    Field::new(name, DataType::Utf8, true)
}

fn decimal(name: &str) -> Field {
    Field::new(
        name,
        DataType::Decimal128(DECIMAL_PRECISION, DECIMAL_SCALE),
        true,
    )
}

fn ts_micros(name: &str) -> Field {
    Field::new(
        name,
        DataType::Timestamp(TimeUnit::Microsecond, Some(TS_UTC.into())),
        true,
    )
}

fn date32(name: &str) -> Field {
    Field::new(name, DataType::Date32, true)
}

fn boolean(name: &str) -> Field {
    Field::new(name, DataType::Boolean, true)
}

// =================================================================
// Builder helpers
// =================================================================

fn decimal_builder() -> Decimal128Builder {
    Decimal128Builder::new()
        .with_precision_and_scale(DECIMAL_PRECISION, DECIMAL_SCALE)
        .unwrap()
}

fn ts_builder() -> TimestampMicrosecondBuilder {
    TimestampMicrosecondBuilder::new()
}

fn finish_decimal(mut builder: Decimal128Builder) -> Result<arrow_array::Decimal128Array> {
    let arr = builder.finish();
    Ok(arr.with_precision_and_scale(DECIMAL_PRECISION, DECIMAL_SCALE)?)
}

fn push_opt_str(builder: &mut StringBuilder, v: Option<&str>) {
    match v {
        Some(s) => builder.append_value(s),
        None => builder.append_null(),
    }
}

fn push_opt_bool(builder: &mut BooleanBuilder, v: Option<bool>) {
    match v {
        Some(b) => builder.append_value(b),
        None => builder.append_null(),
    }
}

fn push_opt_ts(builder: &mut TimestampMicrosecondBuilder, v: Option<DateTime<Utc>>) {
    match v {
        Some(dt) => builder.append_value(datetime_to_micros(dt)),
        None => builder.append_null(),
    }
}

fn push_opt_date(builder: &mut Date32Builder, v: Option<NaiveDate>) {
    match v {
        Some(d) => builder.append_value(date_to_days(d)),
        None => builder.append_null(),
    }
}

fn push_opt_decimal(builder: &mut Decimal128Builder, v: Option<Decimal>) -> Result<()> {
    match v {
        Some(d) => {
            let mantissa = decimal_to_i128(d, DECIMAL_SCALE as u32)
                .with_context(|| format!("decimal {d} cannot be re-scaled to {DECIMAL_SCALE}"))?;
            builder.append_value(mantissa);
            Ok(())
        }
        None => {
            builder.append_null();
            Ok(())
        }
    }
}

fn push_raw_fields(builder: &mut StringBuilder, m: &BTreeMap<String, String>) {
    if m.is_empty() {
        builder.append_null();
    } else {
        builder.append_value(raw_fields_to_json(m));
    }
}

// =================================================================
// Conversion helpers
// =================================================================

fn datetime_to_micros(dt: DateTime<Utc>) -> i64 {
    dt.timestamp_micros()
}

fn date_to_days(d: NaiveDate) -> i32 {
    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).expect("static epoch");
    d.signed_duration_since(epoch).num_days() as i32
}

/// Rescales `d` to `target_scale` and returns its mantissa as `i128`.
/// `Decimal::rescale` mutates in place; the resulting mantissa fits
/// in `i128` because `Decimal` is 96-bit.
fn decimal_to_i128(d: Decimal, target_scale: u32) -> Option<i128> {
    let mut rescaled = d;
    rescaled.rescale(target_scale);
    if rescaled.scale() != target_scale {
        return None;
    }
    Some(rescaled.mantissa())
}

fn raw_fields_to_json(m: &BTreeMap<String, String>) -> String {
    // BTreeMap preserves key order, so the JSON is deterministic.
    serde_json::to_string(m).unwrap_or_else(|_| "{}".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal as D;

    #[test]
    fn emir_schema_field_names_are_unique_and_stable() {
        let s = emir_schema();
        let names: Vec<&str> = s.fields().iter().map(|f| f.name().as_str()).collect();
        // Spot-check critical columns.
        assert!(names.contains(&"uti"));
        assert!(names.contains(&"notional_amount"));
        assert!(names.contains(&"valuation_amount"));
        assert!(names.contains(&"raw_fields"));
        // raw_fields is the last column by convention.
        assert_eq!(names.last(), Some(&"raw_fields"));
        // No duplicates.
        let mut sorted = names.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len(), "no duplicate column names");
    }

    #[test]
    fn sftr_schema_field_names_are_unique_and_stable() {
        let s = sftr_schema();
        let names: Vec<&str> = s.fields().iter().map(|f| f.name().as_str()).collect();
        assert!(names.contains(&"loan_value"));
        assert!(names.contains(&"collateral_value"));
        assert!(names.contains(&"haircut"));
        assert!(names.contains(&"sft_type"));
        assert!(names.contains(&"reuse_indicator"));
        assert_eq!(names.last(), Some(&"raw_fields"));
    }

    #[test]
    fn decimal_roundtrip_positive() {
        let d = D::new(123456789, 5); // 1234.56789
        let m = decimal_to_i128(d, 10).unwrap();
        // 1234.5678900000 → 12345678900000
        assert_eq!(m, 12_345_678_900_000_i128);
    }

    #[test]
    fn decimal_roundtrip_negative() {
        let d = D::new(-100, 2); // -1.00
        let m = decimal_to_i128(d, 10).unwrap();
        // -1.0000000000 → -10000000000
        assert_eq!(m, -10_000_000_000_i128);
    }

    #[test]
    fn datetime_micros_roundtrip() {
        let dt = chrono::DateTime::parse_from_rfc3339("2026-05-14T08:00:00.123456Z")
            .unwrap()
            .with_timezone(&Utc);
        let micros = datetime_to_micros(dt);
        let back = DateTime::<Utc>::from_timestamp_micros(micros).unwrap();
        assert_eq!(back, dt);
    }

    #[test]
    fn date_days_roundtrip() {
        let d = NaiveDate::from_ymd_opt(2026, 5, 14).unwrap();
        let days = date_to_days(d);
        let back =
            NaiveDate::from_ymd_opt(1970, 1, 1).unwrap() + chrono::Duration::days(days as i64);
        assert_eq!(back, d);
    }

    #[test]
    fn raw_fields_json_is_deterministic() {
        let mut m = BTreeMap::new();
        m.insert("foo".to_string(), "bar".to_string());
        m.insert("baz".to_string(), "qux".to_string());
        // BTreeMap iterates in sorted key order → "baz" before "foo".
        let json = raw_fields_to_json(&m);
        assert_eq!(json, r#"{"baz":"qux","foo":"bar"}"#);
    }

    #[test]
    fn empty_records_produces_empty_batch() {
        let schema = emir_schema();
        let records: Vec<EmirRecord> = vec![];
        let batch = build_emir_batch(&schema, &records).unwrap();
        assert_eq!(batch.num_rows(), 0);
        assert_eq!(batch.num_columns(), schema.fields().len());
    }

    #[test]
    fn build_emir_batch_carries_uti_and_notional() {
        let schema = emir_schema();
        let r = EmirRecord {
            uti: Some("UTI-A".into()),
            notional_amount: Some(D::new(100_000_000, 0)), // 100M
            ..Default::default()
        };
        let batch = build_emir_batch(&schema, std::slice::from_ref(&r)).unwrap();
        assert_eq!(batch.num_rows(), 1);
    }

    #[test]
    fn write_emir_parquet_creates_readable_file() {
        let path = std::env::temp_dir().join(format!(
            "opendqi-parquet-{}-emir.parquet",
            std::process::id()
        ));
        let r = EmirRecord {
            uti: Some("UTI-A".into()),
            action_type: Some("NEWT".into()),
            notional_amount: Some(D::new(1_000_000, 0)),
            ..Default::default()
        };
        write_emir_parquet(&path, &[r]).unwrap();
        let meta = std::fs::metadata(&path).unwrap();
        assert!(meta.len() > 0);
        let _ = std::fs::remove_file(&path);
    }
}
