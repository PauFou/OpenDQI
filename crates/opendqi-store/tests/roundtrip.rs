//! Roundtrip: open store, persist a small EMIR + SFTR batch, reload by
//! UTI, assert key fields survive.

use chrono::{NaiveDate, TimeZone, Utc};
use opendqi_core::{
    EmirRecord, FeedbackRecord, FeedbackType, MarginActivityRecord, MarginStateRecord, Regime,
    SftrRecord, SftrTrStateRecord, TrStateRecord,
};
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
    let path = tmp("empty-utis.db");
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

#[test]
fn fresh_open_records_schema_version() {
    let path = tmp("fresh-sv.db");
    let _ = open_store(&path).unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    let mut stmt = conn
        .prepare("SELECT version, name FROM schema_version ORDER BY version")
        .unwrap();
    let rows: Vec<(i32, String)> = stmt
        .query_map([], |row| {
            Ok((row.get::<_, i32>(0)?, row.get::<_, String>(1)?))
        })
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(
        rows,
        vec![
            (1, "initial_schema".to_string()),
            (2, "feedback_validation_rules".to_string()),
        ]
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn legacy_db_is_backfilled() {
    let path = tmp("legacy.db");
    let _ = std::fs::remove_file(&path);
    // Simulate a database created by an older binary: it carries the
    // v1 schema (root `scans` table + `feedbacks` table) but has no
    // `schema_version` table. `feedbacks` must exist so the v1 →
    // legacy-backfill then `m0002` (`ALTER TABLE feedbacks`) path is
    // exercised exactly as on a real upgraded store.
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE scans (scan_id INTEGER PRIMARY KEY);
             CREATE TABLE feedbacks (feedback_id INTEGER PRIMARY KEY);",
        )
        .unwrap();
    }
    // open_store must succeed: record v1 as a legacy backfill (no v1
    // SQL re-run) and then apply m0002 onto the pre-existing feedbacks
    // table.
    let _ = open_store(&path).unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    let (version, name): (i32, String) = conn
        .query_row(
            "SELECT version, name FROM schema_version WHERE version = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(version, 1);
    assert!(name.contains("legacy"), "name was: {name}");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn already_migrated_open_does_not_duplicate_rows() {
    let path = tmp("already-migrated.db");
    let _ = open_store(&path).unwrap();
    let _ = open_store(&path).unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM schema_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        count, 2,
        "second open must not re-insert v1/v2 (one row each)"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn unknown_future_version_does_not_panic() {
    let path = tmp("future-version.db");
    let _ = std::fs::remove_file(&path);
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE schema_version (version INTEGER PRIMARY KEY, name TEXT NOT NULL, applied_at INTEGER NOT NULL);
             INSERT INTO schema_version (version, name, applied_at) VALUES (42, 'future', 0);",
        )
        .unwrap();
    }
    let _ = open_store(&path).unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    let versions: Vec<i32> = conn
        .prepare("SELECT version FROM schema_version ORDER BY version")
        .unwrap()
        .query_map([], |r| r.get::<_, i32>(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    // v1 was applied (since not previously in `applied`) and v42
    // remains as a forward marker — the migrator must tolerate it.
    assert!(versions.contains(&1));
    assert!(versions.contains(&42));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn feedback_persist_list_resolve_workflow() {
    let path = tmp("feedback.db");
    let mut store = open_store(&path).unwrap();

    let batch = vec![
        FeedbackRecord {
            regime: Regime::Emir,
            feedback_type: FeedbackType::Rejected,
            uti: Some("UTI-A".into()),
            reason_code: Some("VAL01".into()),
            reason_description: Some("Invalid currency".into()),
            ..Default::default()
        },
        FeedbackRecord {
            regime: Regime::Emir,
            feedback_type: FeedbackType::Missing,
            uti: Some("UTI-A".into()),
            ..Default::default()
        },
        FeedbackRecord {
            regime: Regime::Sftr,
            feedback_type: FeedbackType::Inaccurate,
            uti: Some("UTI-B".into()),
            reported_field: Some("LoanValue".into()),
            ..Default::default()
        },
    ];
    let inserted = store.persist_feedback_batch(&batch).unwrap();
    assert_eq!(inserted, 3);

    // List all → 3 rows, status=open.
    let all = store.list_feedbacks(None, None, None).unwrap();
    assert_eq!(all.len(), 3);
    assert!(all.iter().all(|r| r.status == "open"));

    // Filter by regime.
    let emir = store
        .list_feedbacks(Some(Regime::Emir), None, None)
        .unwrap();
    assert_eq!(emir.len(), 2);

    // Resolve UTI-A → both EMIR rows resolved.
    let updated = store.update_feedback_status("UTI-A", "resolved").unwrap();
    assert_eq!(updated, 2);

    let open_after = store.list_feedbacks(None, None, Some("open")).unwrap();
    assert_eq!(open_after.len(), 1);
    assert_eq!(open_after[0].uti.as_deref(), Some("UTI-B"));

    let resolved = store.list_feedbacks(None, None, Some("resolved")).unwrap();
    assert_eq!(resolved.len(), 2);

    // Mark UTI-B as stale.
    let updated = store.update_feedback_status("UTI-B", "stale").unwrap();
    assert_eq!(updated, 1);
    let stale = store.list_feedbacks(None, None, Some("stale")).unwrap();
    assert_eq!(stale.len(), 1);

    // Idempotent: re-resolving doesn't update.
    let updated = store.update_feedback_status("UTI-A", "resolved").unwrap();
    assert_eq!(updated, 0);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn feedback_validation_rule_codes_roundtrip_and_fanout() {
    let path = tmp("feedback-rules.db");
    let batch = vec![
        // Multi-rule rejection (auth.092 lists several).
        FeedbackRecord {
            regime: Regime::Emir,
            feedback_type: FeedbackType::Rejected,
            uti: Some("UTI-MULTI".into()),
            reason_code: Some("VR-1".into()),
            validation_rule_codes: vec!["VR-1".into(), "VR-2".into()],
            ..Default::default()
        },
        // No list → falls back to the scalar reason_code in analytics.
        FeedbackRecord {
            regime: Regime::Emir,
            feedback_type: FeedbackType::Rejected,
            uti: Some("UTI-SCALAR".into()),
            reason_code: Some("VR-9".into()),
            ..Default::default()
        },
        // No list, no reason → "(none)".
        FeedbackRecord {
            regime: Regime::Emir,
            feedback_type: FeedbackType::Rejected,
            uti: Some("UTI-NONE".into()),
            ..Default::default()
        },
    ];
    {
        let mut store = open_store(&path).unwrap();
        assert_eq!(store.persist_feedback_batch(&batch).unwrap(), 3);

        let rows = store
            .list_feedbacks(Some(Regime::Emir), None, None)
            .unwrap();
        let multi = rows
            .iter()
            .find(|r| r.uti.as_deref() == Some("UTI-MULTI"))
            .unwrap();
        assert_eq!(multi.validation_rule_codes, vec!["VR-1", "VR-2"]);
        let scalar = rows
            .iter()
            .find(|r| r.uti.as_deref() == Some("UTI-SCALAR"))
            .unwrap();
        assert!(scalar.validation_rule_codes.is_empty());

        // Per-rule fan-out: VR-1 & VR-2 each counted once (from the
        // multi row), VR-9 via scalar fallback, "(none)" for the bare row.
        let by_reason = store.count_feedbacks_by_reason(Some(Regime::Emir)).unwrap();
        let map: std::collections::HashMap<&str, i64> =
            by_reason.iter().map(|(c, n)| (c.as_str(), *n)).collect();
        assert_eq!(map.get("VR-1"), Some(&1));
        assert_eq!(map.get("VR-2"), Some(&1));
        assert_eq!(map.get("VR-9"), Some(&1));
        assert_eq!(map.get("(none)"), Some(&1));
    }
    // Re-open the same DB: the v2 migration is idempotent and the
    // persisted rule list survives.
    {
        let store = open_store(&path).unwrap();
        let rows = store
            .list_feedbacks(Some(Regime::Emir), None, None)
            .unwrap();
        let multi = rows
            .iter()
            .find(|r| r.uti.as_deref() == Some("UTI-MULTI"))
            .unwrap();
        assert_eq!(multi.validation_rule_codes, vec!["VR-1", "VR-2"]);
    }

    let _ = std::fs::remove_file(&path);
}

#[test]
fn tr_state_roundtrip_preserves_fields() {
    let path = tmp("tsr.db");
    let mut store = open_store(&path).unwrap();
    let recs = vec![TrStateRecord {
        record_id: Some("t1".into()),
        regime: Regime::Emir,
        state_as_of: Some(Utc.with_ymd_and_hms(2026, 5, 13, 8, 0, 0).unwrap()),
        uti: Some("UTI-T1".into()),
        status: Some("OUTSTANDING".into()),
        notional_amount: Some(Decimal::from(1_000_000)),
        notional_currency: Some("EUR".into()),
        valuation_amount: Some(Decimal::new(15050, 2)),
        valuation_timestamp: Some(Utc.with_ymd_and_hms(2026, 5, 13, 7, 0, 0).unwrap()),
        maturity_date: NaiveDate::from_ymd_opt(2030, 6, 30),
        collateral_portfolio_code: Some("PORT-1".into()),
        ..Default::default()
    }];
    let scan_id = store.persist_tr_state_batch(1, &recs).unwrap();
    assert!(scan_id > 0);
    let prior = store
        .load_prior_tr_state(&["UTI-T1", "UTI-T9"], i64::MAX)
        .unwrap();
    assert_eq!(prior.len(), 1);
    let p = &prior[0];
    assert_eq!(p.uti.as_deref(), Some("UTI-T1"));
    assert_eq!(p.status.as_deref(), Some("OUTSTANDING"));
    assert_eq!(p.notional_amount.unwrap().to_string(), "1000000");
    assert_eq!(p.valuation_amount.unwrap().to_string(), "150.50");
    assert_eq!(p.collateral_portfolio_code.as_deref(), Some("PORT-1"));
    assert_eq!(p.maturity_date, NaiveDate::from_ymd_opt(2030, 6, 30));

    // exclude filter
    let none = store.load_prior_tr_state(&["UTI-T1"], scan_id).unwrap();
    assert!(none.is_empty());

    let _ = std::fs::remove_file(&path);
}

#[test]
fn sftr_tr_state_roundtrip_preserves_fields() {
    let path = tmp("sftr-tsr.db");
    let mut store = open_store(&path).unwrap();
    let recs = vec![SftrTrStateRecord {
        record_id: Some("s1".into()),
        regime: Regime::Sftr,
        state_as_of: Some(Utc.with_ymd_and_hms(2026, 5, 13, 8, 0, 0).unwrap()),
        uti: Some("SFT-1".into()),
        status: Some("OUTSTANDING".into()),
        sft_type: Some("REPO".into()),
        loan_value: Some(Decimal::from(1_000_000)),
        loan_currency: Some("EUR".into()),
        collateral_value: Some(Decimal::from(1_100_000)),
        collateral_currency: Some("EUR".into()),
        haircut: Some(Decimal::new(5, 2)),
        reuse_indicator: Some(true),
        maturity_date: NaiveDate::from_ymd_opt(2030, 6, 30),
        collateral_isin: Some("FR0000000001".into()),
        ..Default::default()
    }];
    store.persist_sftr_tr_state_batch(1, &recs).unwrap();
    let prior = store
        .load_prior_sftr_tr_state(&["SFT-1"], i64::MAX)
        .unwrap();
    assert_eq!(prior.len(), 1);
    let p = &prior[0];
    assert_eq!(p.uti.as_deref(), Some("SFT-1"));
    assert_eq!(p.sft_type.as_deref(), Some("REPO"));
    assert_eq!(p.reuse_indicator, Some(true));
    assert_eq!(p.haircut.unwrap().to_string(), "0.05");
    assert_eq!(p.collateral_isin.as_deref(), Some("FR0000000001"));

    let _ = std::fs::remove_file(&path);
}

#[test]
fn margin_activity_roundtrip_by_portfolio() {
    let path = tmp("mar.db");
    let mut store = open_store(&path).unwrap();
    let recs = vec![
        MarginActivityRecord {
            record_id: Some("m1".into()),
            uti: Some("UTI-M1".into()),
            action_type: Some("MARU".into()),
            collateral_portfolio_code: Some("PORT-A".into()),
            initial_margin_posted: Some(Decimal::from(1_000_000)),
            margin_currency: Some("EUR".into()),
            event_timestamp: Some(Utc.with_ymd_and_hms(2026, 5, 12, 16, 0, 0).unwrap()),
            reporting_timestamp: Some(Utc.with_ymd_and_hms(2026, 5, 13, 8, 0, 0).unwrap()),
            ..Default::default()
        },
        MarginActivityRecord {
            record_id: Some("m2".into()),
            collateral_portfolio_code: Some("PORT-B".into()),
            variation_margin_posted: Some(Decimal::from(500)),
            ..Default::default()
        },
    ];
    let scan_id = store.persist_margin_activity_batch(1, &recs).unwrap();
    let prior = store
        .load_prior_margin_activity(&["PORT-A"], i64::MAX)
        .unwrap();
    assert_eq!(prior.len(), 1, "filter by portfolio code");
    let p = &prior[0];
    assert_eq!(p.action_type.as_deref(), Some("MARU"));
    assert_eq!(p.initial_margin_posted.unwrap().to_string(), "1000000");
    assert_eq!(p.margin_currency.as_deref(), Some("EUR"));

    // exclude_scan_id
    let none = store
        .load_prior_margin_activity(&["PORT-A"], scan_id)
        .unwrap();
    assert!(none.is_empty());

    let _ = std::fs::remove_file(&path);
}

#[test]
fn margin_state_roundtrip_preserves_fields() {
    let path = tmp("msr.db");
    let mut store = open_store(&path).unwrap();
    let recs = vec![MarginStateRecord {
        record_id: Some("ms1".into()),
        uti: Some("UTI-MS1".into()),
        collateral_portfolio_code: Some("PORT-S".into()),
        initial_margin_posted_current: Some(Decimal::from(1_000_000)),
        initial_margin_collected_current: Some(Decimal::from(1_050_000)),
        collateral_market_value: Some(Decimal::from(1_100_000)),
        haircut_applied: Some(Decimal::new(5, 2)),
        collateralization_category: Some("FCOL".into()),
        state_as_of: Some(Utc.with_ymd_and_hms(2026, 5, 13, 8, 0, 0).unwrap()),
        margin_currency: Some("EUR".into()),
        ..Default::default()
    }];
    store.persist_margin_state_batch(1, &recs).unwrap();
    let prior = store
        .load_prior_margin_state(&["UTI-MS1"], i64::MAX)
        .unwrap();
    assert_eq!(prior.len(), 1);
    let p = &prior[0];
    assert_eq!(p.collateralization_category.as_deref(), Some("FCOL"));
    assert_eq!(p.collateral_market_value.unwrap().to_string(), "1100000");
    assert_eq!(p.haircut_applied.unwrap().to_string(), "0.05");
    assert!(p.state_as_of.is_some());

    let _ = std::fs::remove_file(&path);
}

#[test]
fn empty_input_short_circuits_loaders() {
    let path = tmp("empty.db");
    let store = open_store(&path).unwrap();
    assert!(store.load_prior_tr_state(&[], 0).unwrap().is_empty());
    assert!(store.load_prior_sftr_tr_state(&[], 0).unwrap().is_empty());
    assert!(store.load_prior_margin_activity(&[], 0).unwrap().is_empty());
    assert!(store.load_prior_margin_state(&[], 0).unwrap().is_empty());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn empty_records_persist_creates_scan_row() {
    let path = tmp("empty-scan.db");
    let mut store = open_store(&path).unwrap();
    let scan_id = store.persist_tr_state_batch(1, &[]).unwrap();
    assert!(scan_id > 0);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn load_latest_prior_tr_state_returns_most_recent_snapshot() {
    let path = tmp("latest-tsr.db");
    let mut store = open_store(&path).unwrap();
    // Two prior scans: U1 in scan 1, U1+U2 in scan 2. The third
    // (current) scan should see only the latest (scan 2 → 2 rows).
    let scan1 = store
        .persist_tr_state_batch(
            1,
            &[TrStateRecord {
                uti: Some("U1".into()),
                status: Some("OUTSTANDING".into()),
                ..Default::default()
            }],
        )
        .unwrap();
    let scan2 = store
        .persist_tr_state_batch(
            1,
            &[
                TrStateRecord {
                    uti: Some("U1".into()),
                    status: Some("OUTSTANDING".into()),
                    ..Default::default()
                },
                TrStateRecord {
                    uti: Some("U2".into()),
                    status: Some("OUTSTANDING".into()),
                    ..Default::default()
                },
            ],
        )
        .unwrap();
    let current_scan = scan2 + 1;
    let latest = store.load_latest_prior_tr_state(current_scan).unwrap();
    assert_eq!(latest.len(), 2, "should return scan 2's rows only");
    let uti_set: std::collections::HashSet<&str> =
        latest.iter().filter_map(|r| r.uti.as_deref()).collect();
    assert!(uti_set.contains("U1") && uti_set.contains("U2"));

    // Excluding scan2 leaves scan1 as the most recent prior.
    let prior_of_scan2 = store.load_latest_prior_tr_state(scan2).unwrap();
    assert_eq!(prior_of_scan2.len(), 1);
    assert_eq!(prior_of_scan2[0].uti.as_deref(), Some("U1"));

    // No prior at all → empty.
    let empty = store.load_latest_prior_tr_state(scan1).unwrap();
    assert!(empty.is_empty());

    let _ = std::fs::remove_file(&path);
}

#[test]
fn margin_activity_by_portfolio_returns_empty_when_no_match() {
    let path = tmp("mar-nomatch.db");
    let mut store = open_store(&path).unwrap();
    store
        .persist_margin_activity_batch(
            1,
            &[MarginActivityRecord {
                collateral_portfolio_code: Some("PORT-A".into()),
                initial_margin_posted: Some(Decimal::from(1)),
                ..Default::default()
            }],
        )
        .unwrap();
    let none = store
        .load_prior_margin_activity(&["PORT-OTHER"], i64::MAX)
        .unwrap();
    assert!(none.is_empty());
    let _ = std::fs::remove_file(&path);
}
