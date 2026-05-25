//! End-to-end test for the EMIR `auth.090` (Derivatives Trade
//! Position Set Report) parser against the schema-shaped
//! fixture exercising all 4 position-set kinds.
//!
//! v0.18 E2.

use std::path::PathBuf;
use std::str::FromStr;

use chrono::NaiveDate;
use opendqi_xml::read_emir_position_set_xml;
use rust_decimal::Decimal;

fn example(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join(rel)
}

const FIXTURE: &str = "examples/emir/positions/auth090-sample.xml";

#[test]
fn fixture_parses_5_records_one_per_position_set_element() {
    let outcome = read_emir_position_set_xml(&example(FIXTURE)).expect("parse");
    assert!(
        outcome.issues.is_empty(),
        "expected no format issues, got {:?}",
        outcome.issues
    );
    assert_eq!(outcome.records.len(), 5);
    let kinds: Vec<&str> = outcome
        .records
        .iter()
        .map(|r| r.position_set_kind.as_deref().unwrap())
        .collect();
    assert_eq!(
        kinds,
        vec![
            "PosSet",
            "PosSet",
            "CcyPosSet",
            "CollPosSet",
            "CcyCollPosSet"
        ]
    );
}

#[test]
fn report_ref_date_is_shared_across_every_record() {
    let outcome = read_emir_position_set_xml(&example(FIXTURE)).expect("parse");
    let expected = NaiveDate::from_ymd_opt(2026, 5, 21);
    for (idx, r) in outcome.records.iter().enumerate() {
        assert_eq!(
            r.reference_date,
            expected,
            "record {idx} ({}) should carry the shared RefDt",
            r.position_set_kind.as_deref().unwrap_or("?")
        );
    }
}

#[test]
fn rec_1_posset_credit_swap_full_extraction() {
    let outcome = read_emir_position_set_xml(&example(FIXTURE)).expect("parse");
    let r = &outcome.records[0];
    assert_eq!(
        r.reporting_counterparty.as_deref(),
        Some("RPTGCPARTY0000000001")
    );
    assert_eq!(
        r.other_counterparty.as_deref(),
        Some("OTHRCPARTY0000000002")
    );
    assert_eq!(r.asset_class.as_deref(), Some("CRDT"));
    assert_eq!(r.contract_type.as_deref(), Some("SWAP"));
    assert_eq!(r.value_currency.as_deref(), Some("EUR"));
    assert_eq!(r.underlying_id.as_deref(), Some("DE000A1B2C34"));
    assert_eq!(r.notional, Some(Decimal::from_str("12500000.00").unwrap()));
    assert_eq!(
        r.mark_to_market_value,
        Some(Decimal::from_str("125000.50").unwrap())
    );
    assert!(r.collateral_value.is_none());
}

#[test]
fn rec_4_collposset_captures_collateral_not_notional() {
    let outcome = read_emir_position_set_xml(&example(FIXTURE)).expect("parse");
    let r = &outcome.records[3];
    assert_eq!(r.position_set_kind.as_deref(), Some("CollPosSet"));
    assert!(r.notional.is_none(), "CollPosSet has no Buyr/Ntnl/Amt");
    assert!(
        r.mark_to_market_value.is_none(),
        "CollPosSet has no Buyr/PostvVal"
    );
    assert_eq!(
        r.collateral_value,
        Some(Decimal::from_str("110000.25").unwrap())
    );
}

#[test]
fn rec_5_ccycollposset_captures_collateral_with_currency_bucket() {
    let outcome = read_emir_position_set_xml(&example(FIXTURE)).expect("parse");
    let r = &outcome.records[4];
    assert_eq!(r.position_set_kind.as_deref(), Some("CcyCollPosSet"));
    assert_eq!(r.value_currency.as_deref(), Some("USD"));
    assert_eq!(
        r.collateral_value,
        Some(Decimal::from_str("50000.00").unwrap())
    );
}
