//! THE v1.0 stable Arrow schemas for the Data Quality Pack
//! exports (v0.15 +).
//!
//! Two schemas, two contracts:
//! - [`indicators_schema`] mirrors `indicators.csv` from
//!   `crates/opendqi-report/src/dqi_csv.rs::write_indicators_csv`
//!   column-for-column. 11 columns.
//! - [`evidence_schema`] mirrors `evidence.csv` from the same
//!   module. 7 columns.
//!
//! Both are v1.0 STABLE — any column add/remove/rename/retype
//! bumps the major version of the bindings. Verified at the
//! integration-test level by `tests/test_dqi_pack.py` which
//! loads the CLI goldens with `pyarrow.csv.read_csv` and
//! asserts column name + type equality.

use std::sync::Arc;

use anyhow::Result;
use arrow::array::builder::{Float64Builder, StringBuilder, UInt64Builder};
use arrow::array::{ArrayRef, RecordBatch};
use arrow::datatypes::{DataType, Field, Schema, SchemaRef};

use opendqi_core::{DqiEvidence, DqiIndicator};

/// v1.0 stable Arrow schema for the `indicators.csv` table —
/// 11 columns, matching `opendqi_report::write_indicators_csv`.
///
/// Numeric columns (`numerator`, `denominator`, `rate`,
/// `threshold_amber`, `threshold_red`) are typed natively
/// (`UInt64` / `Float64`) so downstream consumers (pandas,
/// Polars, DuckDB) can aggregate without parsing.
pub fn indicators_schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("indicator_id", DataType::Utf8, false),
        Field::new("regime", DataType::Utf8, false),
        Field::new("dimension", DataType::Utf8, false),
        Field::new("table_scope", DataType::Utf8, false),
        Field::new("numerator", DataType::UInt64, false),
        Field::new("denominator", DataType::UInt64, false),
        Field::new("rate", DataType::Float64, true),
        Field::new("threshold_amber", DataType::Float64, true),
        Field::new("threshold_red", DataType::Float64, true),
        Field::new("status", DataType::Utf8, false),
        Field::new("description", DataType::Utf8, false),
    ]))
}

/// v1.0 stable Arrow schema for the `evidence.csv` table —
/// 7 columns, matching `opendqi_report::write_evidence_csv`.
pub fn evidence_schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("indicator_id", DataType::Utf8, false),
        Field::new("uti", DataType::Utf8, false),
        Field::new("counterparty", DataType::Utf8, true),
        Field::new("asset_class", DataType::Utf8, true),
        Field::new("source_file", DataType::Utf8, true),
        Field::new("observed_value", DataType::Utf8, true),
        Field::new("explanation", DataType::Utf8, false),
    ]))
}

/// Project a slice of [`DqiIndicator`] into a single
/// `RecordBatch` matching [`indicators_schema`].
pub fn indicators_to_record_batch(indicators: &[DqiIndicator]) -> Result<RecordBatch> {
    let schema = indicators_schema();
    let mut indicator_id = StringBuilder::new();
    let mut regime = StringBuilder::new();
    let mut dimension = StringBuilder::new();
    let mut table_scope = StringBuilder::new();
    let mut numerator = UInt64Builder::new();
    let mut denominator = UInt64Builder::new();
    let mut rate = Float64Builder::new();
    let mut threshold_amber = Float64Builder::new();
    let mut threshold_red = Float64Builder::new();
    let mut status = StringBuilder::new();
    let mut description = StringBuilder::new();

    for ind in indicators {
        indicator_id.append_value(&ind.indicator_id);
        regime.append_value(ind.regime.to_string());
        dimension.append_value(ind.dimension.to_string());
        table_scope.append_value(&ind.table_scope);
        numerator.append_value(ind.numerator);
        denominator.append_value(ind.denominator);
        rate.append_option(ind.rate);
        threshold_amber.append_option(ind.threshold_amber);
        threshold_red.append_option(ind.threshold_red);
        status.append_value(ind.status.to_string());
        description.append_value(&ind.description);
    }

    let columns: Vec<ArrayRef> = vec![
        Arc::new(indicator_id.finish()),
        Arc::new(regime.finish()),
        Arc::new(dimension.finish()),
        Arc::new(table_scope.finish()),
        Arc::new(numerator.finish()),
        Arc::new(denominator.finish()),
        Arc::new(rate.finish()),
        Arc::new(threshold_amber.finish()),
        Arc::new(threshold_red.finish()),
        Arc::new(status.finish()),
        Arc::new(description.finish()),
    ];

    Ok(RecordBatch::try_new(schema, columns)?)
}

/// Project a slice of [`DqiEvidence`] into a single
/// `RecordBatch` matching [`evidence_schema`].
pub fn evidence_to_record_batch(evidence: &[DqiEvidence]) -> Result<RecordBatch> {
    let schema = evidence_schema();
    let mut indicator_id = StringBuilder::new();
    let mut uti = StringBuilder::new();
    let mut counterparty = StringBuilder::new();
    let mut asset_class = StringBuilder::new();
    let mut source_file = StringBuilder::new();
    let mut observed_value = StringBuilder::new();
    let mut explanation = StringBuilder::new();

    for ev in evidence {
        indicator_id.append_value(&ev.indicator_id);
        uti.append_value(&ev.uti);
        counterparty.append_option(ev.counterparty.as_deref());
        asset_class.append_option(ev.asset_class.as_deref());
        source_file.append_option(ev.source_file.as_deref());
        observed_value.append_option(ev.observed_value.as_deref());
        explanation.append_value(&ev.explanation);
    }

    let columns: Vec<ArrayRef> = vec![
        Arc::new(indicator_id.finish()),
        Arc::new(uti.finish()),
        Arc::new(counterparty.finish()),
        Arc::new(asset_class.finish()),
        Arc::new(source_file.finish()),
        Arc::new(observed_value.finish()),
        Arc::new(explanation.finish()),
    ];

    Ok(RecordBatch::try_new(schema, columns)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendqi_core::{DqDimension, DqiStatus, Regime};

    fn ind(id: &str, status: DqiStatus) -> DqiIndicator {
        DqiIndicator {
            indicator_id: id.into(),
            regime: Regime::Emir,
            dimension: DqDimension::Completeness,
            table_scope: "TSR".into(),
            numerator: 1,
            denominator: 10,
            rate: Some(0.1),
            threshold_amber: Some(0.05),
            threshold_red: Some(0.2),
            status,
            description: "desc".into(),
        }
    }

    fn ev(id: &str, uti: &str) -> DqiEvidence {
        DqiEvidence {
            indicator_id: id.into(),
            uti: uti.into(),
            counterparty: None,
            asset_class: None,
            source_file: Some("f.xml".into()),
            observed_value: None,
            explanation: "exp".into(),
        }
    }

    #[test]
    fn indicators_schema_is_11_cols_in_csv_order() {
        let s = indicators_schema();
        let names: Vec<&str> = s.fields().iter().map(|f| f.name().as_str()).collect();
        assert_eq!(
            names,
            vec![
                "indicator_id",
                "regime",
                "dimension",
                "table_scope",
                "numerator",
                "denominator",
                "rate",
                "threshold_amber",
                "threshold_red",
                "status",
                "description",
            ]
        );
        let nullables: Vec<bool> = s.fields().iter().map(|f| f.is_nullable()).collect();
        assert_eq!(
            nullables,
            vec![false, false, false, false, false, false, true, true, true, false, false]
        );
    }

    #[test]
    fn evidence_schema_is_7_cols_in_csv_order() {
        let s = evidence_schema();
        let names: Vec<&str> = s.fields().iter().map(|f| f.name().as_str()).collect();
        assert_eq!(
            names,
            vec![
                "indicator_id",
                "uti",
                "counterparty",
                "asset_class",
                "source_file",
                "observed_value",
                "explanation",
            ]
        );
    }

    #[test]
    fn indicators_batch_round_trip() {
        let rows = vec![ind("DQI_A", DqiStatus::Green), ind("DQI_B", DqiStatus::Red)];
        let batch = indicators_to_record_batch(&rows).unwrap();
        assert_eq!(batch.num_rows(), 2);
        assert_eq!(batch.num_columns(), 11);
    }

    #[test]
    fn evidence_batch_round_trip() {
        let rows = vec![ev("DQI_A", "U1"), ev("DQI_A", "U2")];
        let batch = evidence_to_record_batch(&rows).unwrap();
        assert_eq!(batch.num_rows(), 2);
        assert_eq!(batch.num_columns(), 7);
    }

    #[test]
    fn empty_inputs_yield_empty_batches() {
        let i = indicators_to_record_batch(&[]).unwrap();
        assert_eq!(i.num_rows(), 0);
        assert_eq!(i.num_columns(), 11);
        let e = evidence_to_record_batch(&[]).unwrap();
        assert_eq!(e.num_rows(), 0);
        assert_eq!(e.num_columns(), 7);
    }
}
