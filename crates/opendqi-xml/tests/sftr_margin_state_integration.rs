//! End-to-end test for the SFTR `auth.085` (Margin Data
//! Transaction State Report) parser against the schema-shaped
//! fixture (real ESMA SFTR `auth.085.001.02` element paths,
//! synthetic values): parse the XML and verify every reachable
//! `SftrMarginStateRecord` field is populated as expected.
//!
//! v0.17 A2'. Mirrors `sftr_tr_state_integration.rs`.
//! See `docs/auth-messages/sftr-auth085.md` (to be added in I2).

use std::path::PathBuf;
use std::str::FromStr;

use opendqi_xml::read_sftr_margin_state_xml;
use rust_decimal::Decimal;

fn example(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join(rel)
}

#[test]
fn schema_shaped_fixture_parses_5_records_with_no_format_issues() {
    let outcome =
        read_sftr_margin_state_xml(&example("examples/sftr/margin_state/auth085-sample.xml"))
            .expect("parse");
    assert!(
        outcome.issues.is_empty(),
        "expected no format issues, got {:?}",
        outcome.issues
    );
    assert_eq!(outcome.records.len(), 5, "5 <Stat> records");
}

#[test]
fn rec_1_full_record_all_6_amounts_plus_currency_and_metadata() {
    let outcome =
        read_sftr_margin_state_xml(&example("examples/sftr/margin_state/auth085-sample.xml"))
            .expect("parse");
    let r = &outcome.records[0];
    assert_eq!(r.record_id.as_deref(), Some("REC-1"));
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
    assert_eq!(r.action_type.as_deref(), Some("MARU"));
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
fn rec_2_posted_only_received_amounts_stay_none() {
    let outcome =
        read_sftr_margin_state_xml(&example("examples/sftr/margin_state/auth085-sample.xml"))
            .expect("parse");
    let r = &outcome.records[1];
    assert_eq!(r.record_id.as_deref(), Some("REC-2"));
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
fn rec_3_received_only_posted_amounts_stay_none() {
    let outcome =
        read_sftr_margin_state_xml(&example("examples/sftr/margin_state/auth085-sample.xml"))
            .expect("parse");
    let r = &outcome.records[2];
    assert_eq!(r.record_id.as_deref(), Some("REC-3"));
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
fn rec_4_excess_collateral_use_case() {
    // Portfolio-004 has XcssCollPstd = 500000 vs InitlMrgnPstd =
    // 100000 — the excess collateral is 5× the initial margin.
    // This is the "excess collateral use" signature that the
    // future DQI_T3_EXCESS_COLLATERAL_USE_SFTR will detect.
    let outcome =
        read_sftr_margin_state_xml(&example("examples/sftr/margin_state/auth085-sample.xml"))
            .expect("parse");
    let r = &outcome.records[3];
    assert_eq!(r.record_id.as_deref(), Some("REC-4"));
    let im = r.initial_margin_posted.unwrap();
    let xcss = r.excess_collateral_posted.unwrap();
    assert!(xcss > im, "excess collateral should exceed initial margin");
}

#[test]
fn rec_5_natural_person_other_counterparty_plus_negative_amount() {
    let outcome =
        read_sftr_margin_state_xml(&example("examples/sftr/margin_state/auth085-sample.xml"))
            .expect("parse");
    let r = &outcome.records[4];
    assert_eq!(r.record_id.as_deref(), Some("REC-5"));
    // Ntrl/Id/Id path: the natural-person id is captured into
    // other_counterparty (downstream LEI-format checks will flag it).
    assert_eq!(
        r.other_counterparty.as_deref(),
        Some("NATURAL-PERSON-ID-42")
    );
    // Negative amount is parsed verbatim; downstream
    // SFTR.T3.MARGIN_NEGATIVE granular check (F1') will fire.
    assert_eq!(
        r.initial_margin_posted,
        Some(Decimal::from_str("-50.00").unwrap())
    );
}
