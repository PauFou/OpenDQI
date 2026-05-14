//! Round-trip tests for the Parquet writer: ingest a fixture CSV via
//! `read_emir_csv` / `read_sftr_csv`, write a Parquet file, re-read it
//! back with the `parquet` crate's Arrow reader, and verify the key
//! columns survive intact.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use arrow_array::{Array, BooleanArray, Decimal128Array, RecordBatch, StringArray};
use opendqi_core::EmirRecord;
use opendqi_io::{
    read_emir_csv, read_sftr_csv, write_emir_parquet, write_sftr_parquet, CsvMapping,
};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use rust_decimal::Decimal;

fn tmp(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "opendqi-parquet-{}-{name}.parquet",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&p);
    p
}

fn read_first_batch(path: &Path) -> RecordBatch {
    let file = std::fs::File::open(path).unwrap();
    let builder = ParquetRecordBatchReaderBuilder::try_new(file).unwrap();
    let mut reader = builder.build().unwrap();
    reader.next().unwrap().unwrap()
}

fn col_str<'a>(batch: &'a RecordBatch, name: &str) -> &'a StringArray {
    let idx = batch.schema().index_of(name).unwrap();
    batch
        .column(idx)
        .as_any()
        .downcast_ref::<StringArray>()
        .expect("string column")
}

fn col_decimal<'a>(batch: &'a RecordBatch, name: &str) -> &'a Decimal128Array {
    let idx = batch.schema().index_of(name).unwrap();
    batch
        .column(idx)
        .as_any()
        .downcast_ref::<Decimal128Array>()
        .expect("decimal column")
}

fn col_bool<'a>(batch: &'a RecordBatch, name: &str) -> &'a BooleanArray {
    let idx = batch.schema().index_of(name).unwrap();
    batch
        .column(idx)
        .as_any()
        .downcast_ref::<BooleanArray>()
        .expect("bool column")
}

#[test]
fn emir_csv_to_parquet_roundtrip() {
    let mapping = CsvMapping::from_path(Path::new("../../examples/emir/sample_mapping.yml"))
        .expect("load mapping");
    let records =
        read_emir_csv(Path::new("../../examples/emir/sample.csv"), &mapping).expect("load CSV");
    assert!(!records.is_empty(), "fixture must contain records");

    let path = tmp("emir-sample");
    write_emir_parquet(&path, &records).expect("write parquet");

    let batch = read_first_batch(&path);
    assert_eq!(batch.num_rows(), records.len());
    assert!(batch.num_columns() >= 50, "EMIR schema has 50+ columns");

    // Regime literal column.
    let regime = col_str(&batch, "regime");
    assert_eq!(regime.value(0), "EMIR");

    // uti round-trip
    let uti = col_str(&batch, "uti");
    assert_eq!(uti.value(0), records[0].uti.as_deref().unwrap_or(""));

    // notional_amount round-trip (Decimal128 scale=10)
    let notional = col_decimal(&batch, "notional_amount");
    if let Some(expected) = records[0].notional_amount {
        let actual_mantissa = notional.value(0);
        let mut rescaled = expected;
        rescaled.rescale(10);
        assert_eq!(actual_mantissa, rescaled.mantissa());
    }

    let _ = std::fs::remove_file(&path);
}

#[test]
fn sftr_csv_to_parquet_roundtrip() {
    let mapping =
        CsvMapping::from_path(Path::new("../../examples/sftr/tier2.yml")).expect("load mapping");
    let records =
        read_sftr_csv(Path::new("../../examples/sftr/tier2.csv"), &mapping).expect("load CSV");
    assert!(!records.is_empty(), "fixture must contain records");

    let path = tmp("sftr-tier2");
    write_sftr_parquet(&path, &records).expect("write parquet");

    let batch = read_first_batch(&path);
    assert_eq!(batch.num_rows(), records.len());

    let regime = col_str(&batch, "regime");
    assert_eq!(regime.value(0), "SFTR");

    // sft_type → REPO on the fixture's first row.
    let sft_type = col_str(&batch, "sft_type");
    assert_eq!(sft_type.value(0), "REPO");

    // loan_value Decimal128 round-trip.
    let loan = col_decimal(&batch, "loan_value");
    if let Some(expected) = records[0].loan_value {
        let mut rescaled = expected;
        rescaled.rescale(10);
        assert_eq!(loan.value(0), rescaled.mantissa());
    }

    // reuse_indicator bool column exists and is readable.
    let _reuse = col_bool(&batch, "reuse_indicator");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn raw_fields_serialised_as_json() {
    let mut raw = BTreeMap::new();
    raw.insert("foo".to_string(), "bar".to_string());
    raw.insert("baz".to_string(), "qux".to_string());
    let r = EmirRecord {
        uti: Some("UTI-R".into()),
        raw_fields: raw,
        ..Default::default()
    };
    let path = tmp("emir-raw");
    write_emir_parquet(&path, &[r]).unwrap();
    let batch = read_first_batch(&path);
    let raw_col = col_str(&batch, "raw_fields");
    // BTreeMap → JSON in sorted key order.
    assert_eq!(raw_col.value(0), r#"{"baz":"qux","foo":"bar"}"#);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn empty_input_writes_readable_parquet() {
    let path = tmp("empty");
    write_emir_parquet(&path, &[]).unwrap();
    let meta = std::fs::metadata(&path).unwrap();
    assert!(meta.len() > 0, "empty Parquet still has footer + schema");
    let file = std::fs::File::open(&path).unwrap();
    let builder = ParquetRecordBatchReaderBuilder::try_new(file).unwrap();
    // EMIR schema has > 50 columns; we don't pin the exact count to
    // avoid coupling the test to schema growth.
    assert!(builder.schema().fields().len() >= 50);
    let mut reader = builder.build().unwrap();
    let mut total_rows = 0;
    for batch in &mut reader {
        let batch = batch.unwrap();
        total_rows += batch.num_rows();
    }
    assert_eq!(total_rows, 0);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn negative_decimal_roundtrips_through_parquet() {
    let r = EmirRecord {
        uti: Some("UTI-NEG".into()),
        valuation_amount: Some(Decimal::new(-12345, 2)), // -123.45
        ..Default::default()
    };
    let path = tmp("emir-neg");
    write_emir_parquet(&path, std::slice::from_ref(&r)).unwrap();
    let batch = read_first_batch(&path);
    let val = col_decimal(&batch, "valuation_amount");
    let mut expected = r.valuation_amount.unwrap();
    expected.rescale(10);
    assert_eq!(val.value(0), expected.mantissa());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn timestamp_microsecond_resolution_survives() {
    use arrow_array::TimestampMicrosecondArray;
    let dt = chrono::DateTime::parse_from_rfc3339("2026-05-14T08:00:00.123456Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let r = EmirRecord {
        uti: Some("UTI-TS".into()),
        reporting_timestamp: Some(dt),
        ..Default::default()
    };
    let path = tmp("emir-ts");
    write_emir_parquet(&path, &[r]).unwrap();
    let batch = read_first_batch(&path);
    let idx = batch.schema().index_of("reporting_timestamp").unwrap();
    let arr = batch
        .column(idx)
        .as_any()
        .downcast_ref::<TimestampMicrosecondArray>()
        .unwrap();
    let micros = arr.value(0);
    assert_eq!(micros, dt.timestamp_micros());
    let _ = std::fs::remove_file(&path);
}
