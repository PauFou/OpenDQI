//! Roundtrip: open store, persist a small EMIR + SFTR batch, reload by
//! UTI, assert key fields survive.

use chrono::{NaiveDate, TimeZone, Utc};
use opendqi_core::{EmirRecord, SftrRecord};
use opendqi_store::open_store;
use rust_decimal::Decimal;

fn tmp(name: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("opendqi-store-{}-{name}", std::process::id()));
    let _ = std::fs::remove_file(&p);
    p
}

#[test]
fn emir_roundtrip_preserves_lifecycle_fields() {
    let path = tmp("emir.db");
    let mut store = open_store(&path).unwrap();

    let first = vec![EmirRecord {
        record_id: Some("r1".into()),
        uti: Some("UTI-A".into()),
        action_type: Some("NEWT".into()),
        reporting_timestamp: Some(Utc.with_ymd_and_hms(2026, 4, 1, 17, 0, 0).unwrap()),
        valuation_amount: Some(Decimal::new(15050, 2)),
        valuation_timestamp: Some(Utc.with_ymd_and_hms(2026, 4, 1, 16, 0, 0).unwrap()),
        termination_date: None,
        ..Default::default()
    }];
    let scan1 = store.persist_emir_batch(1, &first).unwrap();
    assert!(scan1 > 0);

    // Second scan with a different UTI (shouldn't be returned).
    let second = vec![EmirRecord {
        record_id: Some("r2".into()),
        uti: Some("UTI-B".into()),
        action_type: Some("NEWT".into()),
        ..Default::default()
    }];
    let scan2 = store.persist_emir_batch(1, &second).unwrap();
    assert!(scan2 > scan1);

    // Re-open in read-only-ish mode — same connection, no need.
    let prior = store.load_prior_emir(&["UTI-A", "UTI-C"], 999).unwrap();
    assert_eq!(prior.len(), 1);
    let p = &prior[0];
    assert_eq!(p.uti.as_deref(), Some("UTI-A"));
    assert_eq!(p.action_type.as_deref(), Some("NEWT"));
    assert_eq!(p.valuation_amount.unwrap().to_string(), "150.50");
    assert_eq!(
        p.reporting_timestamp.unwrap(),
        Utc.with_ymd_and_hms(2026, 4, 1, 17, 0, 0).unwrap()
    );

    // exclude_scan_id filters out scans >= cutoff.
    let prior_excl = store.load_prior_emir(&["UTI-A"], scan1).unwrap();
    assert!(prior_excl.is_empty(), "scan_id == cutoff must be excluded");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn sftr_roundtrip_preserves_lifecycle_fields() {
    let path = tmp("sftr.db");
    let mut store = open_store(&path).unwrap();

    let batch = vec![SftrRecord {
        record_id: Some("s1".into()),
        uti: Some("SFT-A".into()),
        action_type: Some("NEWT".into()),
        sft_type: Some("REPO".into()),
        execution_timestamp: Some(Utc.with_ymd_and_hms(2026, 4, 1, 9, 0, 0).unwrap()),
        maturity_date: NaiveDate::from_ymd_opt(2026, 7, 1),
        ..Default::default()
    }];
    let scan_id = store.persist_sftr_batch(1, &batch).unwrap();
    assert!(scan_id > 0);

    let prior = store
        .load_prior_sftr(&["SFT-A", "SFT-X"], scan_id + 1)
        .unwrap();
    assert_eq!(prior.len(), 1);
    let p = &prior[0];
    assert_eq!(p.uti.as_deref(), Some("SFT-A"));
    assert_eq!(p.sft_type.as_deref(), Some("REPO"));
    assert_eq!(p.maturity_date, NaiveDate::from_ymd_opt(2026, 7, 1));

    let _ = std::fs::remove_file(&path);
}

#[test]
fn empty_utis_short_circuit() {
    let path = tmp("empty.db");
    let store = open_store(&path).unwrap();
    assert!(store.load_prior_emir(&[], 1).unwrap().is_empty());
    assert!(store.load_prior_sftr(&[], 1).unwrap().is_empty());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn migrate_is_idempotent() {
    let path = tmp("migrate.db");
    let s1 = open_store(&path).unwrap();
    drop(s1);
    let s2 = open_store(&path).unwrap();
    assert_eq!(s2.count_emir().unwrap(), 0);
    assert_eq!(s2.count_sftr().unwrap(), 0);
    let _ = std::fs::remove_file(&path);
}
