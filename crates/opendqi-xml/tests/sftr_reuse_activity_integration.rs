//! End-to-end test for the SFTR `auth.071` (Reused Collateral
//! Data Report) parser against the schema-shaped fixture (real
//! ESMA SFTR `auth.071.001.02` element paths, synthetic values):
//! parse the XML and verify every reachable
//! `SftrReuseActivityRecord` field is populated as expected,
//! covering all 4 action wrappers (New / Crrctn / CollReuseUpd /
//! Err) and the security-only / cash-only / mixed content
//! variants.
//!
//! v0.18 B2. Mirrors `sftr_margin_activity_integration.rs`.
//! See `docs/auth-messages/sftr-auth071.md` (to be added in
//! Phase H).

use std::path::PathBuf;
use std::str::FromStr;

use opendqi_xml::read_sftr_reuse_activity_xml;
use rust_decimal::Decimal;

fn example(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join(rel)
}

const FIXTURE: &str = "examples/sftr/reuse_activity/auth071-sample.xml";

#[test]
fn schema_shaped_fixture_parses_5_records_with_no_format_issues() {
    let outcome = read_sftr_reuse_activity_xml(&example(FIXTURE)).expect("parse");
    assert!(
        outcome.issues.is_empty(),
        "expected no format issues, got {:?}",
        outcome.issues
    );
    assert_eq!(outcome.records.len(), 5, "5 <Rpt> records");
}

#[test]
fn rec_1_new_wrapper_full_record_sums_actl_and_estmtd_plus_rate() {
    let outcome = read_sftr_reuse_activity_xml(&example(FIXTURE)).expect("parse");
    let r = &outcome.records[0];
    assert_eq!(r.record_id.as_deref(), Some("REUSE-NEW-1"));
    assert_eq!(r.action_type.as_deref(), Some("NEWT"));
    assert_eq!(
        r.reporting_counterparty.as_deref(),
        Some("RPTGCPARTY0000000001")
    );
    assert_eq!(
        r.report_submitting_entity.as_deref(),
        Some("SUBMITRPT000000000001")
    );
    // 1_000_000 (Actl) + 500_000 (Estmtd) = 1_500_000.
    assert_eq!(
        r.total_reuse_value,
        Some(Decimal::from_str("1500000.00").unwrap())
    );
    assert_eq!(r.reuse_currency.as_deref(), Some("EUR"));
    assert_eq!(
        r.cash_reinvestment_rate,
        Some(Decimal::from_str("0.0125").unwrap())
    );
    assert!(r.event_day.is_some());
    assert!(r.state_as_of.is_some());
}

#[test]
fn rec_2_crrctn_wrapper_scty_only_no_cash_rate() {
    let outcome = read_sftr_reuse_activity_xml(&example(FIXTURE)).expect("parse");
    let r = &outcome.records[1];
    assert_eq!(r.record_id.as_deref(), Some("REUSE-CORR-1"));
    assert_eq!(r.action_type.as_deref(), Some("CORR"));
    assert_eq!(
        r.total_reuse_value,
        Some(Decimal::from_str("200000.00").unwrap())
    );
    assert_eq!(r.reuse_currency.as_deref(), Some("USD"));
    assert!(r.cash_reinvestment_rate.is_none());
}

#[test]
fn rec_3_coll_reuse_upd_wrapper_cash_only_no_total_reuse_value() {
    let outcome = read_sftr_reuse_activity_xml(&example(FIXTURE)).expect("parse");
    let r = &outcome.records[2];
    assert_eq!(r.record_id.as_deref(), Some("REUSE-CRUD-1"));
    assert_eq!(r.action_type.as_deref(), Some("CRUD"));
    assert!(
        r.total_reuse_value.is_none(),
        "cash-only record has no Scty entries → no total_reuse_value"
    );
    assert!(
        r.reuse_currency.is_none(),
        "reuse_currency comes from Scty/ReuseVal @Ccy, not Csh"
    );
    assert_eq!(
        r.cash_reinvestment_rate,
        Some(Decimal::from_str("0.0200").unwrap())
    );
}

#[test]
fn rec_4_new_wrapper_large_volume_single_actl() {
    let outcome = read_sftr_reuse_activity_xml(&example(FIXTURE)).expect("parse");
    let r = &outcome.records[3];
    assert_eq!(r.record_id.as_deref(), Some("REUSE-NEW-2"));
    assert_eq!(r.action_type.as_deref(), Some("NEWT"));
    assert_eq!(
        r.total_reuse_value,
        Some(Decimal::from_str("5000000.00").unwrap())
    );
    assert_eq!(r.reuse_currency.as_deref(), Some("EUR"));
    assert!(r.cash_reinvestment_rate.is_none());
}

#[test]
fn rec_5_err_wrapper_metadata_only_no_evtday_no_amounts() {
    let outcome = read_sftr_reuse_activity_xml(&example(FIXTURE)).expect("parse");
    let r = &outcome.records[4];
    assert_eq!(r.record_id.as_deref(), Some("REUSE-ERR-1"));
    assert_eq!(r.action_type.as_deref(), Some("ERRT"));
    assert!(r.event_day.is_none(), "Err wrapper carries no EvtDay");
    assert!(r.total_reuse_value.is_none());
    assert!(r.reuse_currency.is_none());
    assert!(r.cash_reinvestment_rate.is_none());
    assert_eq!(
        r.reporting_counterparty.as_deref(),
        Some("RPTGCPARTY0000000001")
    );
    assert_eq!(
        r.report_submitting_entity.as_deref(),
        Some("SUBMITRPT000000000001")
    );
}
