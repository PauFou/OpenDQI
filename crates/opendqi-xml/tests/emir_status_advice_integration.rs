//! End-to-end test for the EMIR `auth.031` (Financial Instrument
//! Reporting Status Advice) parser against the synthetic fixture.
//!
//! v0.20 A4.

use std::path::PathBuf;

use opendqi_xml::read_emir_status_advice_xml;

fn example(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join(rel)
}

const FIXTURE: &str = "examples/emir/status_advice/auth031-sample.xml";

#[test]
fn fixture_parses_five_acks_one_per_stsadvc_element() {
    let outcome = read_emir_status_advice_xml(&example(FIXTURE)).expect("parse");
    assert!(
        outcome.issues.is_empty(),
        "expected no format issues, got {:?}",
        outcome.issues
    );
    assert_eq!(outcome.records.len(), 5);
}

#[test]
fn fixture_extracts_status_values_in_order() {
    let outcome = read_emir_status_advice_xml(&example(FIXTURE)).expect("parse");
    let statuses: Vec<&str> = outcome
        .records
        .iter()
        .map(|r| r.ack_status.as_deref().unwrap_or(""))
        .collect();
    assert_eq!(statuses, vec!["ACPT", "RJCT", "PDNG", "ACTC", "PRTL"]);
}

#[test]
fn fixture_error_code_present_only_on_rjct_ack() {
    let outcome = read_emir_status_advice_xml(&example(FIXTURE)).expect("parse");
    assert!(outcome.records[0].error_code.is_none(), "ACPT ack");
    assert_eq!(
        outcome.records[1].error_code.as_deref(),
        Some("ERR-3001"),
        "RJCT ack should carry ErrCd"
    );
    assert!(outcome.records[2].error_code.is_none(), "PDNG ack");
}

#[test]
fn fixture_per_ack_timestamp_wins_over_envelope_when_present() {
    let outcome = read_emir_status_advice_xml(&example(FIXTURE)).expect("parse");
    // Ack-3 has its own CreDtTm at 08:30:00, the rest inherit
    // the envelope MsgHdr/CreDtTm at 08:00:00.
    let ack1 = outcome.records[0].ack_timestamp.unwrap();
    let ack3 = outcome.records[2].ack_timestamp.unwrap();
    assert_eq!(ack1.timestamp(), 1779350400); // 2026-05-21T08:00:00Z
    assert_eq!(ack3.timestamp(), 1779352200); // 2026-05-21T08:30:00Z
}

#[test]
fn fixture_submission_ids_are_distinct_per_ack() {
    let outcome = read_emir_status_advice_xml(&example(FIXTURE)).expect("parse");
    let ids: Vec<&str> = outcome
        .records
        .iter()
        .map(|r| r.submission_id.as_deref().unwrap_or(""))
        .collect();
    assert_eq!(
        ids,
        vec![
            "SUB-2026-05-21-001",
            "SUB-2026-05-21-002",
            "SUB-2026-05-21-003",
            "SUB-2026-05-21-004",
            "SUB-2026-05-21-005",
        ]
    );
}
