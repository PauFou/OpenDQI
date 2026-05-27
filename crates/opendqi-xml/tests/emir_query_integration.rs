//! End-to-end test for the EMIR `auth.029` (Derivatives Trade
//! Report Query) parser against the synthetic fixture.
//!
//! v0.20 A2.

use std::path::PathBuf;

use opendqi_xml::read_emir_query_xml;

fn example(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join(rel)
}

const FIXTURE: &str = "examples/emir/query/auth029-sample.xml";

#[test]
fn fixture_parses_one_query_envelope() {
    let outcome = read_emir_query_xml(&example(FIXTURE)).expect("parse");
    assert!(
        outcome.issues.is_empty(),
        "expected no format issues, got {:?}",
        outcome.issues
    );
    // auth.029 is one envelope per file (not a list of records).
    assert_eq!(outcome.records.len(), 1);
}

#[test]
fn fixture_extracts_envelope_typed_fields() {
    let outcome = read_emir_query_xml(&example(FIXTURE)).expect("parse");
    let r = &outcome.records[0];
    assert_eq!(r.query_id.as_deref(), Some("QRY-OPENDQI-2026-001"));
    assert!(r.query_timestamp.is_some(), "query_timestamp should parse");
    assert_eq!(r.requesting_lei.as_deref(), Some("549300ABCDEFGH123456"));
}

#[test]
fn fixture_captures_two_filter_blocks_as_opaque_descriptions() {
    let outcome = read_emir_query_xml(&example(FIXTURE)).expect("parse");
    let r = &outcome.records[0];
    assert_eq!(
        r.filter_descriptions.len(),
        2,
        "expected 2 TradRptQryCrit blocks captured separately"
    );
    let joined: String = r.filter_descriptions.join("|");
    assert!(
        joined.contains("FrDt=2026-05-01"),
        "first block should carry FrDt leaf, got {joined}"
    );
    assert!(
        joined.contains("ToDt=2026-05-21"),
        "first block should carry ToDt leaf, got {joined}"
    );
    assert!(
        joined.contains("AsstClss=CRDT"),
        "second block should carry AsstClss leaf, got {joined}"
    );
}
