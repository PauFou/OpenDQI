//! EMIR Financial Instrument Reporting Status Advice (ISO 20022
//! `auth.031`) ingestion.
//!
//! Element paths are aligned with the
//! `FinancialInstrumentReportingStatusAdviceV01` envelope. The
//! SWIFT-licensed XSD is **not** redistributed; only the schema
//! *shape* (element names, nesting, cardinalities) is encoded
//! here.
//!
//! Real envelope:
//! ```text
//! Document
//! └─ FinInstrmRptgStsAdvc
//!    ├─ MsgHdr
//!    │  └─ CreDtTm        (ack timestamp, ISODateTime; one per envelope)
//!    └─ StsAdvc[]         (one or more per-submission acks)
//!       ├─ OrgnlMsgId     (id of the submission being acked)
//!       ├─ Sts            (status string: ACPT/ACTC/RJCT/PDNG/PRTL)
//!       ├─ ErrCd?         (TR-specific error code, present on RJCT)
//!       └─ CreDtTm?       (per-ack timestamp, falls back to MsgHdr)
//! ```
//!
//! Honest scope: auth.031 is a **status envelope** the TR sends
//! *to* the firm after receiving a submission. It carries no
//! derivatives payload itself — only the identifier of the
//! submission being acked, the status enum, and an optional
//! error code when the submission was rejected.
//!
//! We parse one [`EmirStatusAdviceRecord`] per `StsAdvc` element
//! and promote four typed fields (submission_id, ack_status,
//! ack_timestamp, error_code). Per-ack `CreDtTm` is preferred
//! over the envelope-level `MsgHdr/CreDtTm` when both are present.
//!
//! No business DQ signal beyond the
//! `EMIR.ACK.ENVELOPE_WELLFORMED` sanity check (defined in
//! `opendqi-core::dq::emir_status_advice`). See
//! `docs/auth-messages/emir-auth031.md` for the rationale.

use std::path::Path;

use chrono::{DateTime, Utc};
use opendqi_core::{DqDimension, DqIssue, EmirStatusAdviceRecord, Regime, Severity};
use quick_xml::events::Event;
use quick_xml::name::ResolveResult;
use quick_xml::NsReader;

use crate::wellformed::check_wellformedness;

/// Outcome of reading one EMIR `auth.031` XML file.
#[derive(Debug, Default)]
pub struct EmirStatusAdviceXmlReadOutcome {
    /// Records extracted from the file (one per `StsAdvc`
    /// element inside the envelope).
    pub records: Vec<EmirStatusAdviceRecord>,
    /// File-level data-quality / parse issues.
    pub issues: Vec<DqIssue>,
}

const ISO20022_AUTH_031_NS: &[u8] = b"urn:iso:std:iso:20022:tech:xsd:auth.031.001.01";

/// Read an EMIR `auth.031` Status Advice file.
pub fn read_emir_status_advice_xml(path: &Path) -> anyhow::Result<EmirStatusAdviceXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();

    if let Err(err) = check_wellformedness(path) {
        return Ok(EmirStatusAdviceXmlReadOutcome {
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
        Some(ns) if ns == ISO20022_AUTH_031_NS => parse(path),
        other => {
            let actual = other
                .as_deref()
                .map(|n| String::from_utf8_lossy(n).into_owned())
                .unwrap_or_else(|| "(none)".into());
            Ok(EmirStatusAdviceXmlReadOutcome {
                records: vec![],
                issues: vec![fmt_issue(
                    "EMIR.FMT.XML_UNSUPPORTED_NAMESPACE",
                    Severity::Warning,
                    format!(
                        "Root namespace is '{actual}', expected 'urn:iso:std:iso:20022:tech:xsd:auth.031.001.01'."
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

fn parse(path: &Path) -> anyhow::Result<EmirStatusAdviceXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();
    let mut reader = NsReader::from_file(path)?;
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut pile: Vec<String> = Vec::new();
    let mut is_leaf: Vec<bool> = Vec::new();
    let mut text_buf = String::new();

    // Envelope-level fallback timestamp captured from MsgHdr/CreDtTm
    // before any StsAdvc starts. Per-ack CreDtTm wins when present.
    let mut envelope_ts: Option<DateTime<Utc>> = None;

    let mut current: Option<EmirStatusAdviceRecord> = None;
    let mut rec_depth: Option<usize> = None;
    let mut records: Vec<EmirStatusAdviceRecord> = Vec::new();
    let mut idx_ack: u32 = 0;

    loop {
        match reader.read_resolved_event_into(&mut buf)? {
            (_, Event::Start(e)) => {
                let local = local_name(e.name().as_ref());
                push_element(&mut pile, &mut is_leaf, local.clone());
                text_buf.clear();
                // Detect ack start.
                if current.is_none() && local == "StsAdvc" {
                    idx_ack += 1;
                    current = Some(EmirStatusAdviceRecord {
                        source_file: Some(source_label.clone()),
                        record_id: Some(format!("{source_label}#Ack-{idx_ack}")),
                        regime: Regime::Emir,
                        ack_timestamp: envelope_ts,
                        ..Default::default()
                    });
                    rec_depth = Some(pile.len());
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
                    // Capture envelope timestamp once before any ack opens.
                    if current.is_none()
                        && envelope_ts.is_none()
                        && pile.last().map(String::as_str) == Some("CreDtTm")
                        && pile.iter().any(|s| s == "MsgHdr")
                    {
                        if let Ok(ts) = DateTime::parse_from_rfc3339(trimmed) {
                            envelope_ts = Some(ts.with_timezone(&Utc));
                        }
                    }
                    // In-record leaf commit.
                    if let (Some(rec), Some(rdepth)) = (current.as_mut(), rec_depth) {
                        if pile.len() > rdepth {
                            commit_ack_leaf(rec, &pile[rdepth..], trimmed);
                        }
                        if !trimmed.is_empty() {
                            let key = pile.join("/");
                            rec.raw_fields.insert(key, trimmed.to_owned());
                        }
                    }
                }
                // Close ack record on StsAdvc end.
                if let Some(rdepth) = rec_depth {
                    if pile.len() == rdepth {
                        if let Some(mut rec) = current.take() {
                            // Backfill from envelope if per-ack CreDtTm was absent.
                            if rec.ack_timestamp.is_none() {
                                rec.ack_timestamp = envelope_ts;
                            }
                            records.push(rec);
                        }
                        rec_depth = None;
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

    Ok(EmirStatusAdviceXmlReadOutcome {
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

fn commit_ack_leaf(rec: &mut EmirStatusAdviceRecord, rel: &[String], value: &str) {
    if value.is_empty() {
        return;
    }
    let last = rel.last().map(String::as_str).unwrap_or("");
    match last {
        "OrgnlMsgId" | "MsgId" => {
            rec.submission_id = Some(value.to_owned());
        }
        "Sts" => {
            rec.ack_status = Some(value.to_owned());
        }
        "CreDtTm" => {
            if let Ok(ts) = DateTime::parse_from_rfc3339(value) {
                rec.ack_timestamp = Some(ts.with_timezone(&Utc));
            }
        }
        "ErrCd" => {
            rec.error_code = Some(value.to_owned());
        }
        _ => {}
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

    const FIVE_ACKS: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.031.001.01">
  <FinInstrmRptgStsAdvc>
    <MsgHdr>
      <CreDtTm>2026-05-21T08:00:00Z</CreDtTm>
    </MsgHdr>
    <StsAdvc>
      <OrgnlMsgId>SUB-2026-05-21-001</OrgnlMsgId>
      <Sts>ACPT</Sts>
    </StsAdvc>
    <StsAdvc>
      <OrgnlMsgId>SUB-2026-05-21-002</OrgnlMsgId>
      <Sts>RJCT</Sts>
      <ErrCd>ERR-3001</ErrCd>
    </StsAdvc>
    <StsAdvc>
      <OrgnlMsgId>SUB-2026-05-21-003</OrgnlMsgId>
      <Sts>PDNG</Sts>
      <CreDtTm>2026-05-21T08:30:00Z</CreDtTm>
    </StsAdvc>
    <StsAdvc>
      <OrgnlMsgId>SUB-2026-05-21-004</OrgnlMsgId>
      <Sts>ACTC</Sts>
    </StsAdvc>
    <StsAdvc>
      <OrgnlMsgId>SUB-2026-05-21-005</OrgnlMsgId>
      <Sts>PRTL</Sts>
    </StsAdvc>
  </FinInstrmRptgStsAdvc>
</Document>"#;

    #[test]
    fn parses_five_acks_with_per_ack_status_and_error_code() {
        let p = write_tmp("auth031-five.xml", FIVE_ACKS);
        let outcome = read_emir_status_advice_xml(&p).expect("parse");
        assert!(outcome.issues.is_empty());
        assert_eq!(outcome.records.len(), 5);
        let statuses: Vec<&str> = outcome
            .records
            .iter()
            .map(|r| r.ack_status.as_deref().unwrap_or(""))
            .collect();
        assert_eq!(statuses, vec!["ACPT", "RJCT", "PDNG", "ACTC", "PRTL"]);
        // Error code present on second ack only.
        assert_eq!(outcome.records[1].error_code.as_deref(), Some("ERR-3001"));
        assert!(outcome.records[0].error_code.is_none());
        // Timestamps: per-ack CreDtTm wins on ack 3, envelope on others.
        for r in &outcome.records {
            assert!(r.ack_timestamp.is_some());
        }
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn ack_timestamp_falls_back_to_envelope_when_per_ack_missing() {
        let p = write_tmp("auth031-fallback.xml", FIVE_ACKS);
        let outcome = read_emir_status_advice_xml(&p).expect("parse");
        // Ack 1 has no per-ack CreDtTm → falls back to envelope (08:00:00).
        let ack1_ts = outcome.records[0].ack_timestamp.unwrap();
        assert_eq!(ack1_ts.timestamp(), 1779350400); // 2026-05-21T08:00:00Z
                                                     // Ack 3 has per-ack CreDtTm 08:30:00.
        let ack3_ts = outcome.records[2].ack_timestamp.unwrap();
        assert_eq!(ack3_ts.timestamp(), 1779352200); // 2026-05-21T08:30:00Z
        let _ = std::fs::remove_file(&p);
    }

    const MINIMAL_ACK: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.031.001.01">
  <FinInstrmRptgStsAdvc>
    <StsAdvc>
      <OrgnlMsgId>SUB-X</OrgnlMsgId>
      <Sts>ACPT</Sts>
    </StsAdvc>
  </FinInstrmRptgStsAdvc>
</Document>"#;

    #[test]
    fn ack_without_envelope_timestamp_leaves_ack_timestamp_none() {
        let p = write_tmp("auth031-min.xml", MINIMAL_ACK);
        let outcome = read_emir_status_advice_xml(&p).expect("parse");
        assert_eq!(outcome.records.len(), 1);
        let r = &outcome.records[0];
        assert_eq!(r.submission_id.as_deref(), Some("SUB-X"));
        assert_eq!(r.ack_status.as_deref(), Some("ACPT"));
        assert!(r.ack_timestamp.is_none());
        let _ = std::fs::remove_file(&p);
    }

    const WRONG_NS: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.999.001.01"><X/></Document>"#;

    #[test]
    fn wrong_namespace_emits_warning_no_records() {
        let p = write_tmp("auth031-wrongns.xml", WRONG_NS);
        let outcome = read_emir_status_advice_xml(&p).expect("parse");
        assert!(outcome.records.is_empty());
        assert_eq!(outcome.issues.len(), 1);
        assert_eq!(
            outcome.issues[0].check_id,
            "EMIR.FMT.XML_UNSUPPORTED_NAMESPACE"
        );
        let _ = std::fs::remove_file(&p);
    }
}
