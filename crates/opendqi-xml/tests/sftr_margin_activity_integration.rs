//! End-to-end test for the SFTR `auth.070` (Margin Data
//! Transaction Report) parser against the schema-shaped fixture
//! (real ESMA SFTR `auth.070.001.02` element paths, synthetic
//! values): parse the XML and verify every reachable
//! `SftrMarginActivityRecord` field is populated as expected,
//! covering all 4 action wrappers (New / Crrctn / TradUpd / Err).
//!
//! v0.18 A2. Mirrors `sftr_margin_state_integration.rs`.
//! See `docs/auth-messages/sftr-auth070.md` (to be added in
//! Phase H).

use std::path::PathBuf;
use std::str::FromStr;

use opendqi_xml::read_sftr_margin_activity_xml;
use rust_decimal::Decimal;

fn example(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join(rel)
}

const FIXTURE: &str = "examples/sftr/margin_activity/auth070-sample.xml";

#[test]
fn schema_shaped_fixture_parses_5_records_with_no_format_issues() {
    let outcome = read_sftr_margin_activity_xml(&example(FIXTURE)).expect("parse");
    assert!(
        outcome.issues.is_empty(),
        "expected no format issues, got {:?}",
        outcome.issues
    );
    assert_eq!(outcome.records.len(), 5, "5 <Rpt> records");
}

#[test]
fn rec_1_new_wrapper_full_record_all_6_amounts_action_newt() {
    let outcome = read_sftr_margin_activity_xml(&example(FIXTURE)).expect("parse");
    let r = &outcome.records[0];
    assert_eq!(r.record_id.as_deref(), Some("REC-NEW-1"));
    assert_eq!(r.action_type.as_deref(), Some("NEWT"));
    assert_eq!(
        r.reporting_counterparty.as_deref(),
        Some("RPTGCPARTY0000000001")
    );
    assert_eq!(
        r.other_counterparty.as_deref(),
        Some("OTHRCPARTY0000000002")
    );
    assert_eq!(
        r.collateral_portfolio_code.as_deref(),
        Some("PORTFOLIO-001")
    );
    assert_eq!(r.margin_currency.as_deref(), Some("EUR"));
    assert_eq!(
        r.initial_margin_posted,
        Some(Decimal::from_str("1000000.00").unwrap())
    );
    assert_eq!(
        r.variation_margin_posted,
        Some(Decimal::from_str("50000.00").unwrap())
    );
    assert_eq!(
        r.excess_collateral_posted,
        Some(Decimal::from_str("25000.00").unwrap())
    );
    assert_eq!(
        r.initial_margin_received,
        Some(Decimal::from_str("980000.00").unwrap())
    );
    assert_eq!(
        r.variation_margin_received,
        Some(Decimal::from_str("48000.00").unwrap())
    );
    assert_eq!(
        r.excess_collateral_received,
        Some(Decimal::from_str("20000.00").unwrap())
    );
    assert!(r.event_date.is_some());
    assert!(r.state_as_of.is_some());
}

#[test]
fn rec_2_crrctn_wrapper_posted_only_action_corr() {
    let outcome = read_sftr_margin_activity_xml(&example(FIXTURE)).expect("parse");
    let r = &outcome.records[1];
    assert_eq!(r.record_id.as_deref(), Some("REC-CORR-1"));
    assert_eq!(r.action_type.as_deref(), Some("CORR"));
    assert_eq!(
        r.collateral_portfolio_code.as_deref(),
        Some("PORTFOLIO-002")
    );
    assert_eq!(r.margin_currency.as_deref(), Some("USD"));
    assert!(r.initial_margin_posted.is_some());
    assert!(r.variation_margin_posted.is_some());
    assert!(r.excess_collateral_posted.is_none()); // not in fixture
    assert!(r.initial_margin_received.is_none());
    assert!(r.variation_margin_received.is_none());
    assert!(r.excess_collateral_received.is_none());
}

#[test]
fn rec_3_tradupd_wrapper_received_only_action_trdu() {
    let outcome = read_sftr_margin_activity_xml(&example(FIXTURE)).expect("parse");
    let r = &outcome.records[2];
    assert_eq!(r.record_id.as_deref(), Some("REC-TRD-1"));
    assert_eq!(r.action_type.as_deref(), Some("TRDU"));
    assert_eq!(
        r.collateral_portfolio_code.as_deref(),
        Some("PORTFOLIO-003")
    );
    assert_eq!(r.margin_currency.as_deref(), Some("GBP"));
    assert!(r.initial_margin_posted.is_none());
    assert!(r.variation_margin_posted.is_none());
    assert!(r.excess_collateral_posted.is_none());
    assert!(r.initial_margin_received.is_some());
    assert!(r.variation_margin_received.is_some());
    assert!(r.excess_collateral_received.is_none()); // not in fixture
}

#[test]
fn rec_4_new_wrapper_excess_collateral_signature() {
    // PORTFOLIO-004 has XcssCollPstd = 500000 vs InitlMrgnPstd =
    // 100000 — the excess collateral is 5x the initial margin.
    // This is the activity-side mirror of the
    // DQI_T3_EXCESS_COLLATERAL_USE_SFTR signature (auth.085) that
    // the future A4 DQI_MAR_EXCESS_COLLATERAL_EVENT_RATE_SFTR
    // will detect on auth.070.
    let outcome = read_sftr_margin_activity_xml(&example(FIXTURE)).expect("parse");
    let r = &outcome.records[3];
    assert_eq!(r.record_id.as_deref(), Some("REC-NEW-2"));
    assert_eq!(r.action_type.as_deref(), Some("NEWT"));
    let im = r.initial_margin_posted.unwrap();
    let xcss = r.excess_collateral_posted.unwrap();
    assert!(xcss > im, "excess collateral should exceed initial margin");
}

#[test]
fn rec_5_err_wrapper_metadata_only_no_evtdt_no_amounts_natural_person() {
    let outcome = read_sftr_margin_activity_xml(&example(FIXTURE)).expect("parse");
    let r = &outcome.records[4];
    assert_eq!(r.record_id.as_deref(), Some("REC-ERR-1"));
    assert_eq!(r.action_type.as_deref(), Some("ERRT"));
    // Err wrapper has no EvtDt at the XSD level.
    assert!(r.event_date.is_none(), "Err carries no EvtDt");
    // No amounts either.
    assert!(r.initial_margin_posted.is_none());
    assert!(r.variation_margin_posted.is_none());
    assert!(r.excess_collateral_posted.is_none());
    assert!(r.initial_margin_received.is_none());
    assert!(r.variation_margin_received.is_none());
    assert!(r.excess_collateral_received.is_none());
    assert!(r.margin_currency.is_none());
    // Natural-person OthrCtrPty path (Ntrl/Id/Id) is captured into
    // other_counterparty (downstream LEI-format checks will flag it).
    assert_eq!(
        r.other_counterparty.as_deref(),
        Some("NATURAL-PERSON-ID-42")
    );
    assert_eq!(
        r.collateral_portfolio_code.as_deref(),
        Some("PORTFOLIO-005")
    );
}
