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

// ---------------------------------------------------------------
// v0.22 B6 — edge case tests (audit HIGH #3, robustness on
// pathological inputs). Imports of read_emir_query_xml + PathBuf
// already at top of the file; here we add a local helper to
// write a tmp file per test.
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
fn empty_document_yields_zero_records_no_issues() {
    // Well-formed but containing zero DerivsTradRptQry envelope —
    // parser should silently return an empty record list and no
    // parse-format issues (the absence of the envelope is not an
    // error per the XSD; it just means there is nothing to scan).
    const XML: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.029.001.04">
</Document>"#;
    let p = write_tmp("auth029-empty.xml", XML);
    let outcome = read_emir_query_xml(&p).expect("parse");
    assert!(outcome.records.is_empty(), "expected 0 records");
    assert!(outcome.issues.is_empty(), "expected 0 issues");
    let _ = std::fs::remove_file(&p);
}

#[test]
fn malformed_xml_yields_format_issue() {
    // XML with mismatched tags (opened tag, closed with different
    // name) — wellformedness check fires EMIR.FMT.XML_NOT_WELLFORMED
    // and aborts before parsing. Truncating *after* DerivsTradRptQry
    // opens would still produce a partial record because the parser
    // greedily synthesises one envelope per file on first Start event
    // (defensible behaviour — extract whatever you can); to test the
    // *wellformedness gate* we need a syntactic error the streaming
    // checker catches early.
    const XML: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.029.001.04">
  <DerivsTradRptQry>
    <MsgHdr>
      <MsgId>QRY-BROKEN</MsgId>
    </WrongClosingTag>
  </DerivsTradRptQry>
</Document>"#;
    let p = write_tmp("auth029-malformed.xml", XML);
    let outcome = read_emir_query_xml(&p).expect("read result");
    assert!(outcome.records.is_empty(), "no records on malformed XML");
    assert_eq!(outcome.issues.len(), 1);
    assert_eq!(outcome.issues[0].check_id, "EMIR.FMT.XML_NOT_WELLFORMED");
    let _ = std::fs::remove_file(&p);
}

#[test]
fn utf8_bom_prefix_does_not_break_parse() {
    // Byte-order mark (EF BB BF) prefix is common on
    // Windows-edited XML files. The parser must tolerate it
    // without choking — quick_xml strips the BOM automatically
    // when reading from a file, but make the contract explicit.
    let mut xml = vec![0xEF, 0xBB, 0xBF]; // UTF-8 BOM
    xml.extend_from_slice(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.029.001.04">
  <DerivsTradRptQry>
    <MsgHdr>
      <MsgId>QRY-BOM-1</MsgId>
    </MsgHdr>
    <RqstngPty><Id><LEI>549300BOMBOMBOMB1234</LEI></Id></RqstngPty>
  </DerivsTradRptQry>
</Document>"#,
    );
    let p = write_tmp("auth029-bom.xml", &xml);
    let outcome = read_emir_query_xml(&p).expect("parse");
    assert_eq!(outcome.records.len(), 1);
    assert_eq!(
        outcome.records[0].query_id.as_deref(),
        Some("QRY-BOM-1"),
        "BOM-prefixed XML should still produce records"
    );
    let _ = std::fs::remove_file(&p);
}
