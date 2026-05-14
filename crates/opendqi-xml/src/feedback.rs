//! TR feedback ingestion (ISO 20022 `auth.092` for EMIR and
//! `auth.080` for SFTR).
//!
//! The two messages share the same shape (a header plus a sequence of
//! `<Sts>` blocks, each carrying one of `<Rjctd>` / `<Mssng>` /
//! `<Inaccrt>` / `<RcncltnBrk>`). A single parser handles both;
//! regime selection is driven by the public entry-point.
//!
//! ## Legal note
//!
//! The official SWIFT-licensed XSDs for `auth.092` / `auth.080` are
//! not redistributed with OpenDQI. The fixture under
//! `examples/{emir,sftr}/feedback/` is hand-authored synthetic. If the
//! real schema differs in element names, adapt the leaf table below.

use std::path::Path;

use chrono::{DateTime, Utc};
use opendqi_core::{DqDimension, DqIssue, FeedbackRecord, FeedbackType, Regime, Severity};
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::NsReader;

use crate::wellformed::check_wellformedness;

/// Outcome of reading one TR feedback XML file.
#[derive(Debug, Default)]
pub struct FeedbackXmlReadOutcome {
    /// Records extracted from the file.
    pub records: Vec<FeedbackRecord>,
    /// File-level data-quality issues (format / namespace).
    pub issues: Vec<DqIssue>,
}

const ISO20022_AUTH_092_NS: &[u8] = b"urn:iso:std:iso:20022:tech:xsd:auth.092.001.01";
const ISO20022_AUTH_080_NS: &[u8] = b"urn:iso:std:iso:20022:tech:xsd:auth.080.001.01";

const STS_PARENT: &str = "Sts";

/// Read an EMIR `auth.092` feedback file.
pub fn read_emir_feedback_xml(path: &Path) -> anyhow::Result<FeedbackXmlReadOutcome> {
    read_with_regime(path, Regime::Emir, ISO20022_AUTH_092_NS, "auth.092.001.01")
}

/// Read an SFTR `auth.080` feedback file.
pub fn read_sftr_feedback_xml(path: &Path) -> anyhow::Result<FeedbackXmlReadOutcome> {
    read_with_regime(path, Regime::Sftr, ISO20022_AUTH_080_NS, "auth.080.001.01")
}

fn read_with_regime(
    path: &Path,
    regime: Regime,
    expected_ns: &[u8],
    expected_label: &str,
) -> anyhow::Result<FeedbackXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();
    let (check_wf, check_ns) = match regime {
        Regime::Emir => (
            "EMIR.FMT.XML_NOT_WELLFORMED",
            "EMIR.FMT.XML_UNSUPPORTED_NAMESPACE",
        ),
        Regime::Sftr => (
            "SFTR.FMT.XML_NOT_WELLFORMED",
            "SFTR.FMT.XML_UNSUPPORTED_NAMESPACE",
        ),
    };

    if let Err(err) = check_wellformedness(path) {
        return Ok(FeedbackXmlReadOutcome {
            records: vec![],
            issues: vec![DqIssue {
                check_id: check_wf.into(),
                regime,
                severity: Severity::Critical,
                dimension: DqDimension::Validity,
                record_id: None,
                uti: None,
                field: None,
                value: None,
                message: format!("XML is not well-formed: {}", err.message),
                source_file: Some(source_label),
                evidence: Vec::new(),
            }],
        });
    }

    match peek_root_namespace(path)? {
        Some(ns) if ns == expected_ns => parse(path, regime),
        other => {
            let actual = other
                .as_deref()
                .map(|n| String::from_utf8_lossy(n).into_owned())
                .unwrap_or_else(|| "(none)".into());
            Ok(FeedbackXmlReadOutcome {
                records: vec![],
                issues: vec![DqIssue {
                    check_id: check_ns.into(),
                    regime,
                    severity: Severity::Warning,
                    dimension: DqDimension::Validity,
                    record_id: None,
                    uti: None,
                    field: None,
                    value: None,
                    message: format!(
                        "Root namespace is '{actual}', expected 'urn:iso:std:iso:20022:tech:xsd:{expected_label}'."
                    ),
                    source_file: Some(source_label),
                    evidence: Vec::new(),
                }],
            })
        }
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

fn parse(path: &Path, regime: Regime) -> anyhow::Result<FeedbackXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();
    let mut reader = NsReader::from_file(path)?;
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut pile: Vec<String> = Vec::new();
    let mut is_leaf: Vec<bool> = Vec::new();
    let mut text_buf = String::new();

    let mut header_timestamp: Option<DateTime<Utc>> = None;
    let mut current: Option<FeedbackRecord> = None;
    let mut sts_depth: Option<usize> = None;
    let mut records: Vec<FeedbackRecord> = Vec::new();
    let mut sts_index: u32 = 0;

    loop {
        match reader.read_resolved_event_into(&mut buf)? {
            (_, Event::Start(e)) => {
                let local = local_name(&e);
                push_element(&mut pile, &mut is_leaf, local.clone());
                text_buf.clear();

                // Entering a new <Sts> block.
                if current.is_none() && pile.last().map(String::as_str) == Some(STS_PARENT) {
                    sts_index += 1;
                    current = Some(FeedbackRecord {
                        source_file: Some(source_label.clone()),
                        record_id: Some(format!("{source_label}#sts-{sts_index}")),
                        regime,
                        feedback_timestamp: header_timestamp,
                        ..Default::default()
                    });
                    sts_depth = Some(pile.len());
                    continue;
                }

                // Inside an <Sts> block: detect the wrapper that sets feedback_type.
                if let (Some(rec), Some(sdepth)) = (current.as_mut(), sts_depth) {
                    if pile.len() == sdepth + 1 {
                        if let Some(ft) = wrapper_to_feedback_type(&local) {
                            rec.feedback_type = ft;
                        }
                    }
                }
            }
            (_, Event::Empty(e)) => {
                let local = local_name(&e);
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
                    // Header timestamp lives outside <Sts>.
                    if current.is_none()
                        && pile.ends_with(&["Hdr".into(), "FdbckDtTm".into()])
                        && !trimmed.is_empty()
                    {
                        if let Ok(dt) = DateTime::parse_from_rfc3339(trimmed) {
                            header_timestamp = Some(dt.with_timezone(&Utc));
                        }
                    }
                    // Leaves inside a <Sts> block.
                    if let (Some(rec), Some(sdepth)) = (current.as_mut(), sts_depth) {
                        if pile.len() > sdepth + 1 {
                            let leaf = pile.last().map(String::as_str).unwrap_or("");
                            commit_leaf(rec, leaf, trimmed);
                        }
                    }
                }

                // Leaving the <Sts> block.
                if let Some(sdepth) = sts_depth {
                    if pile.len() == sdepth {
                        if let Some(mut rec) = current.take() {
                            if rec.feedback_timestamp.is_none() {
                                rec.feedback_timestamp = header_timestamp;
                            }
                            records.push(rec);
                        }
                        sts_depth = None;
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

    Ok(FeedbackXmlReadOutcome {
        records,
        issues: Vec::new(),
    })
}

fn wrapper_to_feedback_type(local: &str) -> Option<FeedbackType> {
    Some(match local {
        "Rjctd" => FeedbackType::Rejected,
        "Mssng" => FeedbackType::Missing,
        "Inaccrt" => FeedbackType::Inaccurate,
        "RcncltnBrk" => FeedbackType::ReconciliationBreak,
        _ => return None,
    })
}

fn commit_leaf(rec: &mut FeedbackRecord, leaf: &str, value: &str) {
    if value.is_empty() {
        return;
    }
    match leaf {
        "UnqTxIdr" => rec.uti = Some(value.to_owned()),
        "RsnCd" => rec.reason_code = Some(value.to_owned()),
        "RsnDesc" => rec.reason_description = Some(value.to_owned()),
        "FldNm" => rec.reported_field = Some(value.to_owned()),
        _ => {}
    }
}

fn push_element(pile: &mut Vec<String>, is_leaf: &mut Vec<bool>, local: String) {
    pile.push(local);
    is_leaf.push(true);
    let n = is_leaf.len();
    if n >= 2 {
        is_leaf[n - 2] = false;
    }
}

fn pop_element(pile: &mut Vec<String>, is_leaf: &mut Vec<bool>) {
    pile.pop();
    is_leaf.pop();
}

fn local_name(e: &BytesStart<'_>) -> String {
    String::from_utf8_lossy(e.local_name().as_ref()).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_tmp(name: &str, content: &[u8]) -> std::path::PathBuf {
        let p =
            std::env::temp_dir().join(format!("opendqi-feedback-{}-{name}", std::process::id()));
        std::fs::write(&p, content).unwrap();
        p
    }

    #[test]
    fn parses_emir_feedback() {
        let body = br#"<?xml version="1.0" encoding="UTF-8"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.092.001.01">
  <FeedbackToReportingMembers>
    <Hdr><FdbckDtTm>2026-05-13T08:00:00Z</FdbckDtTm></Hdr>
    <Sts><Rjctd><UnqTxIdr>U1</UnqTxIdr><RsnCd>VAL01</RsnCd></Rjctd></Sts>
    <Sts><Mssng><UnqTxIdr>U2</UnqTxIdr></Mssng></Sts>
    <Sts><Inaccrt><UnqTxIdr>U3</UnqTxIdr><FldNm>Notional</FldNm></Inaccrt></Sts>
    <Sts><RcncltnBrk><UnqTxIdr>U4</UnqTxIdr></RcncltnBrk></Sts>
  </FeedbackToReportingMembers>
</Document>"#;
        let p = write_tmp("emir.xml", body);
        let out = read_emir_feedback_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert_eq!(out.records.len(), 4);
        assert_eq!(out.records[0].feedback_type, FeedbackType::Rejected);
        assert_eq!(out.records[0].uti.as_deref(), Some("U1"));
        assert_eq!(out.records[0].reason_code.as_deref(), Some("VAL01"));
        assert_eq!(out.records[1].feedback_type, FeedbackType::Missing);
        assert_eq!(out.records[2].feedback_type, FeedbackType::Inaccurate);
        assert_eq!(out.records[2].reported_field.as_deref(), Some("Notional"));
        assert_eq!(
            out.records[3].feedback_type,
            FeedbackType::ReconciliationBreak
        );
        // Header timestamp propagated.
        assert!(out.records[0].feedback_timestamp.is_some());
    }

    #[test]
    fn unsupported_namespace_yields_warning() {
        let body = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.030.001.03"/>"#;
        let p = write_tmp("wrong-ns.xml", body);
        let out = read_emir_feedback_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert!(out.records.is_empty());
        assert_eq!(out.issues.len(), 1);
        assert_eq!(out.issues[0].check_id, "EMIR.FMT.XML_UNSUPPORTED_NAMESPACE");
    }

    #[test]
    fn malformed_xml_yields_critical() {
        let body = br#"<?xml version="1.0"?><Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.092.001.01"><Sts><Rjctd"#;
        let p = write_tmp("bad.xml", body);
        let out = read_emir_feedback_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert!(out.records.is_empty());
        assert_eq!(out.issues[0].severity, Severity::Critical);
    }

    #[test]
    fn parses_sftr_feedback() {
        let body = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.080.001.01">
  <FeedbackToReportingMembers>
    <Sts><Mssng><UnqTxIdr>S-MISSING</UnqTxIdr></Mssng></Sts>
  </FeedbackToReportingMembers>
</Document>"#;
        let p = write_tmp("sftr.xml", body);
        let out = read_sftr_feedback_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert_eq!(out.records.len(), 1);
        assert_eq!(out.records[0].regime, Regime::Sftr);
        assert_eq!(out.records[0].feedback_type, FeedbackType::Missing);
    }
}
