//! CSV ingestion against the canonical EMIR model.

use std::path::Path;
use std::str::FromStr;

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use opendqi_core::EmirRecord;
use rust_decimal::Decimal;
use tracing::warn;

use crate::mapping::CsvMapping;

/// Read a CSV file and produce a `Vec<EmirRecord>` using the given mapping.
///
/// Empty or unparseable cells are left as `None` rather than failing the
/// whole load — the data-quality checks downstream will surface missing
/// values as issues.
pub fn read_emir_csv(path: &Path, mapping: &CsvMapping) -> Result<Vec<EmirRecord>> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(path)
        .with_context(|| format!("opening CSV {}", path.display()))?;

    let headers = reader
        .headers()
        .cloned()
        .with_context(|| format!("reading CSV headers from {}", path.display()))?;
    let source_label = path.to_string_lossy().into_owned();

    let mut out = Vec::new();
    for (i, row) in reader.records().enumerate() {
        let row = row.with_context(|| format!("reading row {} from {}", i + 1, path.display()))?;

        let pick = |canonical: &str| -> Option<String> {
            let header = mapping.fields.get(canonical)?;
            let col_idx = headers.iter().position(|h| h == header)?;
            let raw = row.get(col_idx)?;
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_owned())
            }
        };

        let line_no = (i + 2) as u64; // +1 for 0-based, +1 for header line
        let record_id_label = format!("{}#{line_no}", source_label);

        let date_fmt = mapping.effective_date_format();
        let dt_fmt = mapping.effective_datetime_format();

        let record = EmirRecord {
            source_file: Some(source_label.clone()),
            record_id: pick("record_id").or_else(|| Some(record_id_label.clone())),
            uti: pick("uti"),
            prior_uti: pick("prior_uti"),
            action_type: pick("action_type"),
            event_type: pick("event_type"),
            entity_responsible_for_reporting: pick("entity_responsible_for_reporting"),
            counterparty_1: pick("counterparty_1"),
            counterparty_2: pick("counterparty_2"),
            asset_class: pick("asset_class"),
            product_id: pick("product_id"),
            underlying_id: pick("underlying_id"),
            notional_amount: pick("notional_amount")
                .and_then(|v| parse_decimal(&v, "notional_amount", &record_id_label)),
            notional_currency: pick("notional_currency"),
            price: pick("price").and_then(|v| parse_decimal(&v, "price", &record_id_label)),
            price_currency: pick("price_currency"),
            execution_timestamp: pick("execution_timestamp")
                .and_then(|v| parse_datetime(&v, dt_fmt, "execution_timestamp", &record_id_label)),
            event_timestamp: pick("event_timestamp")
                .and_then(|v| parse_datetime(&v, dt_fmt, "event_timestamp", &record_id_label)),
            reporting_timestamp: pick("reporting_timestamp")
                .and_then(|v| parse_datetime(&v, dt_fmt, "reporting_timestamp", &record_id_label)),
            effective_date: pick("effective_date")
                .and_then(|v| parse_date(&v, date_fmt, "effective_date", &record_id_label)),
            maturity_date: pick("maturity_date")
                .and_then(|v| parse_date(&v, date_fmt, "maturity_date", &record_id_label)),
            termination_date: pick("termination_date")
                .and_then(|v| parse_date(&v, date_fmt, "termination_date", &record_id_label)),
            valuation_amount: pick("valuation_amount")
                .and_then(|v| parse_decimal(&v, "valuation_amount", &record_id_label)),
            valuation_currency: pick("valuation_currency"),
            valuation_timestamp: pick("valuation_timestamp")
                .and_then(|v| parse_datetime(&v, dt_fmt, "valuation_timestamp", &record_id_label)),
            initial_margin_posted: pick("initial_margin_posted")
                .and_then(|v| parse_decimal(&v, "initial_margin_posted", &record_id_label)),
            initial_margin_collected: pick("initial_margin_collected")
                .and_then(|v| parse_decimal(&v, "initial_margin_collected", &record_id_label)),
            variation_margin_posted: pick("variation_margin_posted")
                .and_then(|v| parse_decimal(&v, "variation_margin_posted", &record_id_label)),
            variation_margin_collected: pick("variation_margin_collected")
                .and_then(|v| parse_decimal(&v, "variation_margin_collected", &record_id_label)),
            collateral_portfolio_code: pick("collateral_portfolio_code"),
            clearing_status: pick("clearing_status"),
            collateralisation_category: pick("collateralisation_category"),
            clearing_ccp_lei: pick("clearing_ccp_lei"),
            nature: pick("nature"),
            corporate_sector: pick("corporate_sector"),
            trading_capacity: pick("trading_capacity"),
            valuation_type: pick("valuation_type"),
            master_agreement_type: pick("master_agreement_type"),
            master_agreement_version: pick("master_agreement_version"),
            intragroup_indicator: pick("intragroup_indicator").and_then(|v| parse_bool(&v)),
            hedging_indicator: pick("hedging_indicator").and_then(|v| parse_bool(&v)),
            leg2_notional_amount: pick("leg2_notional_amount")
                .and_then(|v| parse_decimal(&v, "leg2_notional_amount", &record_id_label)),
            leg2_notional_currency: pick("leg2_notional_currency"),
            leg1_payment_frequency: pick("leg1_payment_frequency"),
            leg2_payment_frequency: pick("leg2_payment_frequency"),
            mtm_value_change: pick("mtm_value_change")
                .and_then(|v| parse_decimal(&v, "mtm_value_change", &record_id_label)),
            delta: pick("delta").and_then(|v| parse_decimal(&v, "delta", &record_id_label)),
            gamma: pick("gamma").and_then(|v| parse_decimal(&v, "gamma", &record_id_label)),
            vega: pick("vega").and_then(|v| parse_decimal(&v, "vega", &record_id_label)),
            ..Default::default()
        };
        out.push(record);
    }
    Ok(out)
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "y" | "yes" => Some(true),
        "false" | "0" | "n" | "no" => Some(false),
        _ => None,
    }
}

fn parse_decimal(value: &str, field: &str, record_id: &str) -> Option<Decimal> {
    match Decimal::from_str(value) {
        Ok(d) => Some(d),
        Err(e) => {
            warn!(record = %record_id, field, value, error = %e, "could not parse decimal");
            None
        }
    }
}

fn parse_date(value: &str, fmt: &str, field: &str, record_id: &str) -> Option<NaiveDate> {
    match NaiveDate::parse_from_str(value, fmt) {
        Ok(d) => Some(d),
        Err(_) => match NaiveDate::parse_from_str(value, "%Y-%m-%d") {
            Ok(d) => Some(d),
            Err(e) => {
                warn!(record = %record_id, field, value, error = %e, "could not parse date");
                None
            }
        },
    }
}

fn parse_datetime(value: &str, fmt: &str, field: &str, record_id: &str) -> Option<DateTime<Utc>> {
    if let Ok(dt) = DateTime::parse_from_str(value, fmt) {
        return Some(dt.with_timezone(&Utc));
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(value) {
        return Some(dt.with_timezone(&Utc));
    }
    warn!(record = %record_id, field, value, "could not parse datetime");
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use tempfile_like::NamedTempFile;

    mod tempfile_like {
        use std::path::{Path, PathBuf};

        pub struct NamedTempFile {
            path: PathBuf,
        }

        impl NamedTempFile {
            pub fn new(name: &str) -> Self {
                let dir = std::env::temp_dir();
                let path = dir.join(format!("opendqi-test-{}-{name}", std::process::id()));
                let _ = std::fs::remove_file(&path);
                Self { path }
            }
            pub fn path(&self) -> &Path {
                &self.path
            }
            pub fn write_all(&mut self, data: &[u8]) -> std::io::Result<()> {
                std::fs::write(&self.path, data)
            }
        }

        impl Drop for NamedTempFile {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.path);
            }
        }
    }

    fn make_mapping() -> CsvMapping {
        let mut fields = BTreeMap::new();
        fields.insert("uti".into(), "trade_uti".into());
        fields.insert("notional_amount".into(), "notional".into());
        fields.insert("notional_currency".into(), "notional_ccy".into());
        fields.insert("maturity_date".into(), "maturity".into());
        CsvMapping {
            fields,
            date_format: None,
            datetime_format: None,
        }
    }

    #[test]
    fn reads_simple_csv() {
        let mut f = NamedTempFile::new("simple.csv");
        f.write_all(b"trade_uti,notional,notional_ccy,maturity\nOPENDQI-1,1000000,EUR,2030-12-31\n,500,USD,2099-12-31\n").unwrap();
        let records = read_emir_csv(f.path(), &make_mapping()).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].uti.as_deref(), Some("OPENDQI-1"));
        assert_eq!(
            records[0].notional_amount.map(|d| d.to_string()),
            Some("1000000".into())
        );
        assert_eq!(records[1].uti, None);
    }

    #[test]
    fn unmapped_column_yields_none() {
        let mut f = NamedTempFile::new("unmapped.csv");
        f.write_all(
            b"trade_uti,notional,notional_ccy,maturity\nOPENDQI-1,1000000,EUR,2030-12-31\n",
        )
        .unwrap();
        let records = read_emir_csv(f.path(), &make_mapping()).unwrap();
        assert_eq!(records[0].counterparty_1, None);
        assert_eq!(records[0].reporting_timestamp, None);
    }
}
