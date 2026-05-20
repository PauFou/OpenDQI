//! THE v1.0 stable Arrow schema for `DqIssue` exports.
//!
//! Column order **matches**
//! `crates/opendqi-report/src/csv_out.rs::write_issues_csv`
//! byte-for-byte. Any change to either side breaks the contract
//! and bumps the major version of the bindings.
//!
//! Verified at integration-test time by
//! `tests/test_issues_schema.py` which loads the existing CLI
//! goldens with `pyarrow.csv.read_csv` and asserts column name +
//! type equality against `issues_schema()`.

use std::sync::Arc;

use anyhow::Result;
use arrow::array::builder::StringBuilder;
use arrow::array::{ArrayRef, RecordBatch};
use arrow::datatypes::{DataType, Field, Schema, SchemaRef};

use opendqi_core::DqIssue;

/// Build the v1.0 stable Arrow `Schema` for `DqIssue` outputs.
///
/// 11 columns, all `Utf8`. Column **order** is the contract — it
/// matches the CSV column order in
/// `crates/opendqi-report/src/csv_out.rs`. Nullability mirrors the
/// `Option<T>` pattern on `DqIssue`.
pub fn issues_schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("check_id", DataType::Utf8, false),
        Field::new("regime", DataType::Utf8, false),
        Field::new("severity", DataType::Utf8, false),
        Field::new("dimension", DataType::Utf8, false),
        Field::new("record_id", DataType::Utf8, true),
        Field::new("uti", DataType::Utf8, true),
        Field::new("field", DataType::Utf8, true),
        Field::new("value", DataType::Utf8, true),
        Field::new("message", DataType::Utf8, false),
        Field::new("source_file", DataType::Utf8, true),
        Field::new("evidence_json", DataType::Utf8, true),
    ]))
}

/// Project an iterator of `DqIssue` into a single Arrow
/// `RecordBatch` matching `issues_schema()`. The iterator is
/// consumed eagerly — the bindings collect into Arrow before
/// returning to Python (P5 may add a streaming variant once
/// Arrow C Data Stream wiring exists; out of scope here).
pub fn issues_to_record_batch<I>(issues: I) -> Result<RecordBatch>
where
    I: IntoIterator<Item = DqIssue>,
{
    let schema = issues_schema();

    // 11 builders, in the schema column order. StringBuilder
    // handles `Option<&str>` nullability natively via
    // `append_option`.
    let mut check_id = StringBuilder::new();
    let mut regime = StringBuilder::new();
    let mut severity = StringBuilder::new();
    let mut dimension = StringBuilder::new();
    let mut record_id = StringBuilder::new();
    let mut uti = StringBuilder::new();
    let mut field = StringBuilder::new();
    let mut value = StringBuilder::new();
    let mut message = StringBuilder::new();
    let mut source_file = StringBuilder::new();
    let mut evidence_json = StringBuilder::new();

    for issue in issues {
        check_id.append_value(&issue.check_id);
        regime.append_value(issue.regime.to_string());
        severity.append_value(issue.severity.to_string());
        dimension.append_value(issue.dimension.to_string());
        record_id.append_option(issue.record_id.as_deref());
        uti.append_option(issue.uti.as_deref());
        field.append_option(issue.field.as_deref());
        value.append_option(issue.value.as_deref());
        message.append_value(&issue.message);
        source_file.append_option(issue.source_file.as_deref());

        // `evidence` is `Vec<EvidenceItem>` — serialise to JSON so
        // it round-trips cleanly to the existing `issues.csv`
        // `evidence_json` column (csv_out.rs uses the same
        // serialisation). Empty vectors serialise to "[]" but we
        // emit NULL in that case to keep the CSV/Arrow byte
        // parity (CSV writer leaves the cell empty when the vec
        // is empty — see csv_out.rs:65).
        if issue.evidence.is_empty() {
            evidence_json.append_null();
        } else {
            let serialised = serde_json::to_string(&issue.evidence)?;
            evidence_json.append_value(&serialised);
        }
    }

    let columns: Vec<ArrayRef> = vec![
        Arc::new(check_id.finish()),
        Arc::new(regime.finish()),
        Arc::new(severity.finish()),
        Arc::new(dimension.finish()),
        Arc::new(record_id.finish()),
        Arc::new(uti.finish()),
        Arc::new(field.finish()),
        Arc::new(value.finish()),
        Arc::new(message.finish()),
        Arc::new(source_file.finish()),
        Arc::new(evidence_json.finish()),
    ];

    Ok(RecordBatch::try_new(schema, columns)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendqi_core::{DqDimension, Regime, Severity};

    fn issue(check_id: &str) -> DqIssue {
        DqIssue {
            check_id: check_id.to_string(),
            regime: Regime::Emir,
            severity: Severity::High,
            dimension: DqDimension::Completeness,
            record_id: Some("R1".to_string()),
            uti: Some("U1".to_string()),
            field: Some("uti".to_string()),
            value: None,
            message: "test message".to_string(),
            evidence: vec![],
            source_file: Some("/tmp/x.xml".to_string()),
        }
    }

    #[test]
    fn schema_has_11_cols_in_canonical_order() {
        let s = issues_schema();
        let names: Vec<&str> = s.fields().iter().map(|f| f.name().as_str()).collect();
        assert_eq!(
            names,
            vec![
                "check_id",
                "regime",
                "severity",
                "dimension",
                "record_id",
                "uti",
                "field",
                "value",
                "message",
                "source_file",
                "evidence_json",
            ]
        );
        // Required columns are non-nullable; optional fields are nullable.
        let nullables: Vec<bool> = s.fields().iter().map(|f| f.is_nullable()).collect();
        assert_eq!(
            nullables,
            vec![false, false, false, false, true, true, true, true, false, true, true]
        );
    }

    #[test]
    fn batch_has_correct_row_count_and_columns() {
        let batch = issues_to_record_batch(vec![issue("X"), issue("Y")]).unwrap();
        assert_eq!(batch.num_rows(), 2);
        assert_eq!(batch.num_columns(), 11);
    }

    #[test]
    fn empty_iterator_yields_empty_batch() {
        let batch = issues_to_record_batch(Vec::<DqIssue>::new()).unwrap();
        assert_eq!(batch.num_rows(), 0);
        assert_eq!(batch.num_columns(), 11);
    }
}
