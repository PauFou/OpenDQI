//! End-to-end test for the SFTR `auth.086` (Reused Collateral
//! Data Transaction State Report) parser against the schema-
//! shaped fixture (real ESMA SFTR `auth.086.001.02` element
//! paths, synthetic values).
//!
//! v0.18 C2. Mirror of `sftr_reuse_activity_integration.rs`
//! adapted to the state-block envelope (no action wrappers,
//! action_type from CtrctMod/ActnTp leaf).

use std::path::PathBuf;
use std::str::FromStr;

use opendqi_xml::read_sftr_reuse_state_xml;
use rust_decimal::Decimal;

fn example(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join(rel)
}

const FIXTURE: &str = "examples/sftr/reuse_state/auth086-sample.xml";

#[test]
fn schema_shaped_fixture_parses_4_records_with_no_format_issues() {
    let outcome = read_sftr_reuse_state_xml(&example(FIXTURE)).expect("parse");
    assert!(
        outcome.issues.is_empty(),
        "expected no format issues, got {:?}",
        outcome.issues
    );
    assert_eq!(outcome.records.len(), 4, "4 <Stat> records");
}

#[test]
fn rec_1_full_state_scty_plus_cash_action_reuu() {
    let outcome = read_sftr_reuse_state_xml(&example(FIXTURE)).expect("parse");
    let r = &outcome.records[0];
    assert_eq!(r.record_id.as_deref(), Some("R-STATE-1"));
    assert_eq!(r.action_type.as_deref(), Some("REUU"));
    assert_eq!(
        r.reporting_counterparty.as_deref(),
        Some("RPTGCPARTY0000000001")
    );
    assert_eq!(
        r.report_submitting_entity.as_deref(),
        Some("SUBMITRPT000000000001")
    );
    assert_eq!(
        r.total_reuse_value,
        Some(Decimal::from_str("1000000.00").unwrap())
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
fn rec_2_scty_only_state_no_cash_rate() {
    let outcome = read_sftr_reuse_state_xml(&example(FIXTURE)).expect("parse");
    let r = &outcome.records[1];
    assert_eq!(r.record_id.as_deref(), Some("R-STATE-2"));
    assert_eq!(
        r.total_reuse_value,
        Some(Decimal::from_str("500000.00").unwrap())
    );
    assert_eq!(r.reuse_currency.as_deref(), Some("USD"));
    assert!(r.cash_reinvestment_rate.is_none());
}

#[test]
fn rec_3_cash_only_state_rate_set_no_total_reuse_value() {
    let outcome = read_sftr_reuse_state_xml(&example(FIXTURE)).expect("parse");
    let r = &outcome.records[2];
    assert_eq!(r.record_id.as_deref(), Some("R-STATE-3"));
    assert!(r.total_reuse_value.is_none());
    assert!(r.reuse_currency.is_none());
    assert_eq!(
        r.cash_reinvestment_rate,
        Some(Decimal::from_str("0.0200").unwrap())
    );
}

#[test]
fn rec_4_multi_scty_sums_actl_and_estmtd_uniformly() {
    let outcome = read_sftr_reuse_state_xml(&example(FIXTURE)).expect("parse");
    let r = &outcome.records[3];
    assert_eq!(r.record_id.as_deref(), Some("R-STATE-4"));
    // 100_000 + 200_000 + 50_000 = 350_000.
    assert_eq!(
        r.total_reuse_value,
        Some(Decimal::from_str("350000.00").unwrap())
    );
    assert_eq!(r.reuse_currency.as_deref(), Some("EUR"));
}
