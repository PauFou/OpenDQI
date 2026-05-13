//! End-to-end test against the bundled synthetic auth.030 fixture.
//!
//! No subprocess required — runs entirely in-process against the
//! adapter.

use std::path::PathBuf;

use opendqi_xml::read_emir_xml;

fn fixture_path() -> PathBuf {
    let crate_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(crate_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join("examples/emir/iso20022/sample.xml")
}

#[test]
fn fixture_extracts_eleven_records_with_eight_action_types() {
    let outcome = read_emir_xml(&fixture_path()).expect("parse");
    assert_eq!(
        outcome.records.len(),
        11,
        "expected 11 records, got {}",
        outcome.records.len()
    );

    let mut actions: Vec<&str> = outcome
        .records
        .iter()
        .filter_map(|r| r.action_type.as_deref())
        .collect();
    actions.sort();
    actions.dedup();
    // Expect at least NEWT (the bulk), MODI, CORR, ETRM, MARU, VALU.
    for code in ["NEWT", "MODI", "CORR", "ETRM", "MARU", "VALU"] {
        assert!(
            actions.contains(&code),
            "expected action {code} in {actions:?}",
        );
    }
}

#[test]
fn fixture_does_not_emit_unsupported_namespace() {
    let outcome = read_emir_xml(&fixture_path()).expect("parse");
    assert!(
        outcome
            .issues
            .iter()
            .all(|i| i.check_id != "EMIR.FMT.XML_UNSUPPORTED_NAMESPACE"),
        "got namespace warning: {:?}",
        outcome.issues
    );
}

#[test]
fn margin_update_record_has_four_margin_fields() {
    let outcome = read_emir_xml(&fixture_path()).expect("parse");
    let margin = outcome
        .records
        .iter()
        .find(|r| r.action_type.as_deref() == Some("MARU"))
        .expect("MARU record");
    assert!(margin.initial_margin_posted.is_some(), "initial posted");
    assert!(
        margin.initial_margin_collected.is_some(),
        "initial collected"
    );
    assert!(margin.variation_margin_posted.is_some(), "variation posted");
    assert!(
        margin.variation_margin_collected.is_some(),
        "variation collected"
    );
}

#[test]
fn swap_record_has_two_legs() {
    let outcome = read_emir_xml(&fixture_path()).expect("parse");
    let clean = outcome
        .records
        .iter()
        .find(|r| r.uti.as_deref() == Some("OPENDQI-ISO-CLEAN-0001"))
        .expect("clean record");
    assert_eq!(clean.notional_currency.as_deref(), Some("EUR"));
    assert_eq!(clean.leg2_notional_currency.as_deref(), Some("USD"));
    assert!(clean.leg2_notional_amount.is_some());
    assert!(clean.leg1_payment_frequency.is_some());
    assert!(clean.leg2_payment_frequency.is_some());
}

#[test]
fn clean_record_carries_clearing_and_master_agreement() {
    let outcome = read_emir_xml(&fixture_path()).expect("parse");
    let clean = outcome
        .records
        .iter()
        .find(|r| r.uti.as_deref() == Some("OPENDQI-ISO-CLEAN-0001"))
        .expect("clean record");
    assert_eq!(clean.clearing_status.as_deref(), Some("CLRD"));
    assert_eq!(
        clean.clearing_ccp_lei.as_deref(),
        Some("LCHLDNUS00000000AA")
    );
    assert_eq!(clean.master_agreement_type.as_deref(), Some("ISDA"));
    assert_eq!(clean.master_agreement_version.as_deref(), Some("2002"));
    assert_eq!(clean.valuation_type.as_deref(), Some("MTMA"));
    assert_eq!(clean.intragroup_indicator, Some(false));
    assert_eq!(clean.hedging_indicator, Some(true));
}

#[test]
fn raw_fields_capture_non_routed_leaves() {
    let outcome = read_emir_xml(&fixture_path()).expect("parse");
    // The MrgnUpd record's <PrtflCd> is a direct child of MrgnUpd, not
    // under CmonTradData/Coll, so it is intentionally not in the
    // typed-routing table. It must land in raw_fields.
    let margin = outcome
        .records
        .iter()
        .find(|r| r.action_type.as_deref() == Some("MARU"))
        .expect("MARU record");
    assert_eq!(
        margin.raw_fields.get("PrtflCd").map(String::as_str),
        Some("PORT-001"),
        "expected PrtflCd in raw_fields, got {:?}",
        margin.raw_fields
    );
}

#[test]
fn modification_record_carries_prior_uti() {
    let outcome = read_emir_xml(&fixture_path()).expect("parse");
    let modi = outcome
        .records
        .iter()
        .find(|r| r.action_type.as_deref() == Some("MODI"))
        .expect("MODI record");
    assert_eq!(modi.prior_uti.as_deref(), Some("OPENDQI-ISO-DUPE-0006"));
}
