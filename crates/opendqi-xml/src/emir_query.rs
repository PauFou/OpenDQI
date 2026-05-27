//! EMIR Derivatives Trade Report Query (ISO 20022 `auth.029`)
//! ingestion.
//!
//! Element paths are aligned with the
//! `DerivativesTradeReportQueryV04` envelope. The SWIFT-licensed
//! XSD is **not** redistributed; only the schema *shape* (element
//! names, nesting, cardinalities) is encoded here.
//!
//! Real envelope:
//! ```text
//! Document
//! └─ DerivsTradRptQry
//!    ├─ MsgHdr
//!    │  ├─ MsgId       (query id, string)
//!    │  └─ CreDtTm     (request timestamp, ISODateTime)
//!    ├─ TradRptQryCrit[]  (one or more filter blocks)
//!    │  └─ …               (date range, UTI list, CP filters, …)
//!    └─ RqstngPty
//!       └─ Id/.../LEI  (LEI of the requesting firm)
//! ```
//!
//! Honest scope: auth.029 is a **request envelope** the firm
//! sends *to* the TR to retrieve its own derivatives data. It
//! carries no derivatives payload itself — only the identifying
//! information about the query (who, when, what filters).
//!
//! We therefore parse one record per file (the entire
//! `DerivsTradRptQry` is the single record), populate three
//! typed fields (query_id, query_timestamp, requesting_lei),
//! collect each `TradRptQryCrit` block as one opaque
//! `filter_descriptions` string, and stash everything else into
//! `raw_fields` for downstream inspection.
//!
//! No business DQ signal beyond the
//! `EMIR.QRY.ENVELOPE_WELLFORMED` sanity check (defined in
//! `opendqi-core::dq::emir_query`). See
//! `docs/auth-messages/emir-auth029.md` for the rationale.

use std::path::Path;

use chrono::{DateTime, Utc};
use opendqi_core::{DqDimension, DqIssue, EmirQueryRecord, Regime, Severity};
use quick_xml::events::Event;
use quick_xml::name::ResolveResult;
use quick_xml::NsReader;

use crate::wellformed::check_wellformedness;

/// Outcome of reading one EMIR `auth.029` XML file.
#[derive(Debug, Default)]
pub struct EmirQueryXmlReadOutcome {
    /// Records extracted from the file. auth.029 contains exactly
    /// one `DerivsTradRptQry` envelope per file (the message is a
    /// single query, not a list), so this vector has length 0 or 1.
    pub records: Vec<EmirQueryRecord>,
    /// File-level data-quality / parse issues (wellformedness,
    /// namespace mismatch).
    pub issues: Vec<DqIssue>,
}

const ISO20022_AUTH_029_NS: &[u8] = b"urn:iso:std:iso:20022:tech:xsd:auth.029.001.04";

/// Read an EMIR `auth.029` Trade Report Query file.
pub fn read_emir_query_xml(path: &Path) -> anyhow::Result<EmirQueryXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();

    if let Err(err) = check_wellformedness(path) {
        return Ok(EmirQueryXmlReadOutcome {
            records: vec![],
            issues: vec![fmt_issue(
                "EMIR.FMT.XML_NOT_WELLFORMED",
                Severity::Critical,
                format!("XML is not well-formed: {}", err.message),
                source_label,
            )],
        });
    }

    match peek_root_namespace(path)? {
        Some(ns) if ns == ISO20022_AUTH_029_NS => parse(path),
        other => {
            let actual = other
                .as_deref()
                .map(|n| String::from_utf8_lossy(n).into_owned())
                .unwrap_or_else(|| "(none)".into());
            Ok(EmirQueryXmlReadOutcome {
                records: vec![],
                issues: vec![fmt_issue(
                    "EMIR.FMT.XML_UNSUPPORTED_NAMESPACE",
                    Severity::Warning,
                    format!(
                        "Root namespace is '{actual}', expected 'urn:iso:std:iso:20022:tech:xsd:auth.029.001.04'."
                    ),
                    source_label,
                )],
            })
        }
    }
}

fn fmt_issue(check_id: &str, severity: Severity, message: String, source_file: String) -> DqIssue {
    DqIssue {
        check_id: check_id.into(),
        regime: Regime::Emir,
        severity,
        dimension: DqDimension::Validity,
        record_id: None,
        uti: None,
        field: None,
        value: None,
        message,
        source_file: Some(source_file),
        evidence: Vec::new(),
    }
}

fn peek_root_namespace(path: &Path) -> anyhow::Result<Option<Vec<u8>>> {
    let mut reader = NsReader::from_file(path)?;
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    loop {
        match reader.read_resolved_event_into(&mut buf)? {
            (ns, Event::Start(_)) | (ns, Event::Empty(_)) => {
                return Ok(match ns {
                    ResolveResult::Bound(n) => Some(n.as_ref().to_owned()),
                    _ => None,
                });
            }
            (_, Event::Eof) => return Ok(None),
            _ => {}
        }
        buf.clear();
    }
}

fn parse(path: &Path) -> anyhow::Result<EmirQueryXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();
    let mut reader = NsReader::from_file(path)?;
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut pile: Vec<String> = Vec::new();
    let mut is_leaf: Vec<bool> = Vec::new();
    let mut text_buf = String::new();

    // Single record per file: build it lazily, commit at end if at
    // least one envelope field landed.
    let mut rec = EmirQueryRecord {
        source_file: Some(source_label.clone()),
        record_id: Some(format!("{source_label}#Qry-1")),
        regime: Regime::Emir,
        ..Default::default()
    };
    // Track depth at which we entered TradRptQryCrit so we can
    // synthesise one filter_descriptions entry per block.
    let mut crit_entry_depth: Option<usize> = None;
    let mut current_crit_buf: Vec<String> = Vec::new();
    let mut saw_envelope = false;

    loop {
        match reader.read_resolved_event_into(&mut buf)? {
            (_, Event::Start(e)) => {
                let local = local_name(e.name().as_ref());
                push_element(&mut pile, &mut is_leaf, local.clone());
                text_buf.clear();
                if local == "DerivsTradRptQry" {
                    saw_envelope = true;
                }
                if local == "TradRptQryCrit" && crit_entry_depth.is_none() {
                    crit_entry_depth = Some(pile.len());
                    current_crit_buf.clear();
                }
            }
            (_, Event::Empty(e)) => {
                let local = local_name(e.name().as_ref());
                push_element(&mut pile, &mut is_leaf, local);
                pop_element(&mut pile, &mut is_leaf);
                text_buf.clear();
            }
            (_, Event::Text(t)) => {
                if let Ok(s) = t.unescape() {
                    text_buf.push_str(&s);
                }
            }
            (_, Event::CData(t)) => {
                if let Ok(s) = std::str::from_utf8(t.as_ref()) {
                    text_buf.push_str(s);
                }
            }
            (_, Event::End(_)) => {
                let leaf_now = is_leaf.last().copied().unwrap_or(false);
                if leaf_now {
                    let trimmed = text_buf.trim();
                    commit_envelope_leaf(&mut rec, &pile, trimmed);
                    // Inside a TradRptQryCrit subtree, capture each
                    // leaf as `tail/leaf=value` for the opaque
                    // filter_descriptions string.
                    if let Some(depth) = crit_entry_depth {
                        if pile.len() > depth && !trimmed.is_empty() {
                            let rel = pile[depth..].join("/");
                            current_crit_buf.push(format!("{rel}={trimmed}"));
                        }
                    }
                    // Always stash in raw_fields catch-all.
                    if !trimmed.is_empty() {
                        let key = pile.join("/");
                        rec.raw_fields.insert(key, trimmed.to_owned());
                    }
                }
                // Close TradRptQryCrit block: flush accumulated
                // entries as one filter_descriptions string.
                if let Some(depth) = crit_entry_depth {
                    if pile.len() == depth {
                        if !current_crit_buf.is_empty() {
                            rec.filter_descriptions.push(current_crit_buf.join(";"));
                        }
                        crit_entry_depth = None;
                        current_crit_buf.clear();
                    }
                }
                pop_element(&mut pile, &mut is_leaf);
                text_buf.clear();
            }
            (_, Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }

    let records = if saw_envelope { vec![rec] } else { vec![] };
    Ok(EmirQueryXmlReadOutcome {
        records,
        issues: Vec::new(),
    })
}

fn local_name(name: &[u8]) -> String {
    let s = std::str::from_utf8(name).unwrap_or("");
    match s.rfind(':') {
        Some(i) => s[i + 1..].to_owned(),
        None => s.to_owned(),
    }
}

fn push_element(pile: &mut Vec<String>, is_leaf: &mut Vec<bool>, name: String) {
    // Mark the parent (if any) as no longer a leaf.
    if let Some(last) = is_leaf.last_mut() {
        *last = false;
    }
    pile.push(name);
    is_leaf.push(true);
}

fn pop_element(pile: &mut Vec<String>, is_leaf: &mut Vec<bool>) {
    pile.pop();
    is_leaf.pop();
}

fn commit_envelope_leaf(rec: &mut EmirQueryRecord, pile: &[String], value: &str) {
    if value.is_empty() {
        return;
    }
    let last = pile.last().map(String::as_str).unwrap_or("");
    // MsgHdr/MsgId — query identifier.
    if last == "MsgId" && pile.iter().any(|s| s == "MsgHdr") {
        rec.query_id = Some(value.to_owned());
        return;
    }
    // MsgHdr/CreDtTm — request timestamp (ISODateTime).
    if last == "CreDtTm" && pile.iter().any(|s| s == "MsgHdr") {
        if let Ok(ts) = DateTime::parse_from_rfc3339(value) {
            rec.query_timestamp = Some(ts.with_timezone(&Utc));
        }
        return;
    }
    // RqstngPty/.../LEI — requesting firm LEI.
    if last == "LEI" && pile.iter().any(|s| s == "RqstngPty") {
        rec.requesting_lei = Some(value.to_owned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::write;
    use std::path::PathBuf;

    fn write_tmp(name: &str, content: &[u8]) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("opendqi-test-{}-{}", std::process::id(), name));
        write(&p, content).unwrap();
        p
    }

    const FULL_QUERY: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.029.001.04">
  <DerivsTradRptQry>
    <MsgHdr>
      <MsgId>QRY-2026-001</MsgId>
      <CreDtTm>2026-05-21T08:00:00Z</CreDtTm>
    </MsgHdr>
    <TradRptQryCrit>
      <TradDt>
        <FrDt>2026-05-01</FrDt>
        <ToDt>2026-05-21</ToDt>
      </TradDt>
    </TradRptQryCrit>
    <TradRptQryCrit>
      <AsstClss>CRDT</AsstClss>
    </TradRptQryCrit>
    <RqstngPty>
      <Id>
        <LEI>549300ABCDEFGH123456</LEI>
      </Id>
    </RqstngPty>
  </DerivsTradRptQry>
</Document>"#;

    #[test]
    fn parses_full_query_with_typed_fields_and_two_filter_blocks() {
        let p = write_tmp("auth029-full.xml", FULL_QUERY);
        let outcome = read_emir_query_xml(&p).expect("parse");
        assert_eq!(outcome.records.len(), 1);
        let r = &outcome.records[0];
        assert_eq!(r.regime, Regime::Emir);
        assert_eq!(r.query_id.as_deref(), Some("QRY-2026-001"));
        assert!(r.query_timestamp.is_some());
        assert_eq!(r.requesting_lei.as_deref(), Some("549300ABCDEFGH123456"));
        assert_eq!(r.filter_descriptions.len(), 2);
        assert!(r.filter_descriptions[0].contains("FrDt=2026-05-01"));
        assert!(r.filter_descriptions[1].contains("AsstClss=CRDT"));
        assert!(outcome.issues.is_empty());
        let _ = std::fs::remove_file(&p);
    }

    const MINIMAL_QUERY: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.029.001.04">
  <DerivsTradRptQry>
    <RqstngPty><Id><LEI>549300XYZ12345678901</LEI></Id></RqstngPty>
  </DerivsTradRptQry>
</Document>"#;

    #[test]
    fn parses_minimal_query_only_lei_present() {
        let p = write_tmp("auth029-min.xml", MINIMAL_QUERY);
        let outcome = read_emir_query_xml(&p).expect("parse");
        assert_eq!(outcome.records.len(), 1);
        let r = &outcome.records[0];
        assert!(r.query_id.is_none());
        assert!(r.query_timestamp.is_none());
        assert_eq!(r.requesting_lei.as_deref(), Some("549300XYZ12345678901"));
        assert!(r.filter_descriptions.is_empty());
        let _ = std::fs::remove_file(&p);
    }

    const WRONG_NS: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.999.001.04"><X/></Document>"#;

    #[test]
    fn wrong_namespace_emits_warning_no_records() {
        let p = write_tmp("auth029-wrongns.xml", WRONG_NS);
        let outcome = read_emir_query_xml(&p).expect("parse");
        assert!(outcome.records.is_empty());
        assert_eq!(outcome.issues.len(), 1);
        assert_eq!(
            outcome.issues[0].check_id,
            "EMIR.FMT.XML_UNSUPPORTED_NAMESPACE"
        );
        let _ = std::fs::remove_file(&p);
    }
}
