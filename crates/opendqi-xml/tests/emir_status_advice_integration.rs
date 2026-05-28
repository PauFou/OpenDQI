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

// ---------------------------------------------------------------
// v0.22 B6 — edge case tests (audit HIGH #3, robustness on
// pathological inputs). Imports of read_emir_status_advice_xml +
// PathBuf already at top of the file; here we add a local helper
// to write a tmp file per test.
// ---------------------------------------------------------------

use std::fs::write;

fn write_tmp(name: &str, content: &[u8]) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "opendqi-v022-{}-{name}",
        std::process::id()
    ));
    write(&p, content).unwrap();
    p
}

#[test]
fn zero_stsadvc_yields_empty_record_list() {
    // FinInstrmRptgStsAdvc envelope present but no StsAdvc
    // children — valid per XSD (just an empty batch).
    const XML: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.031.001.01">
  <FinInstrmRptgStsAdvc>
    <MsgHdr>
      <CreDtTm>2026-05-21T08:00:00Z</CreDtTm>
    </MsgHdr>
  </FinInstrmRptgStsAdvc>
</Document>"#;
    let p = write_tmp("auth031-empty.xml", XML);
    let outcome = read_emir_status_advice_xml(&p).expect("parse");
    assert!(outcome.records.is_empty(), "expected 0 records");
    assert!(outcome.issues.is_empty(), "expected 0 issues");
    let _ = std::fs::remove_file(&p);
}

#[test]
fn malformed_namespace_yields_unsupported_warning() {
    // Namespace not matching auth.031.001.01 — parser should
    // emit EMIR.FMT.XML_UNSUPPORTED_NAMESPACE and return zero
    // records (not crash, not silently accept).
    const XML: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.999.001.99">
  <X/>
</Document>"#;
    let p = write_tmp("auth031-wrongns.xml", XML);
    let outcome = read_emir_status_advice_xml(&p).expect("parse");
    assert!(outcome.records.is_empty());
    assert_eq!(outcome.issues.len(), 1);
    assert_eq!(
        outcome.issues[0].check_id,
        "EMIR.FMT.XML_UNSUPPORTED_NAMESPACE"
    );
    let _ = std::fs::remove_file(&p);
}

#[test]
fn large_volume_1000_acks_parses_without_panic() {
    // Generate a 1000-ack envelope in-memory to validate the
    // parser scales linearly and produces the right record
    // count. Catches accidental quadratic behaviour or recursion
    // depth issues that smoke fixtures (5 acks) would miss.
    let mut xml = String::from(
        r#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.031.001.01">
  <FinInstrmRptgStsAdvc>
    <MsgHdr><CreDtTm>2026-05-21T08:00:00Z</CreDtTm></MsgHdr>
"#,
    );
    for i in 0..1000 {
        xml.push_str(&format!(
            "    <StsAdvc><OrgnlMsgId>SUB-{i:05}</OrgnlMsgId><Sts>ACPT</Sts></StsAdvc>\n"
        ));
    }
    xml.push_str("  </FinInstrmRptgStsAdvc>\n</Document>\n");
    let p = write_tmp("auth031-1000.xml", xml.as_bytes());
    let outcome = read_emir_status_advice_xml(&p).expect("parse");
    assert_eq!(outcome.records.len(), 1000, "expected exactly 1000 acks");
    assert!(outcome.issues.is_empty(), "no parse-format issues");
    // First and last record sanity check.
    assert_eq!(
        outcome.records[0].submission_id.as_deref(),
        Some("SUB-00000")
    );
    assert_eq!(
        outcome.records[999].submission_id.as_deref(),
        Some("SUB-00999")
    );
    let _ = std::fs::remove_file(&p);
}
