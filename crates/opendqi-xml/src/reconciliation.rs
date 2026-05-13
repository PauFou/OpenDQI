//! TR reconciliation ingestion (ISO 20022 `auth.106` for EMIR and
//! `auth.083` for SFTR).
//!
//! A reconciliation message is a list of `<Rcncltn>` blocks. Each
//! block carries one UTI, the two counterparty LEIs, a pairing status
//! (`PAIRED` / `UNPAIRED`), a reconciliation status (`RECONCILED` /
//! `UNRECONCILED`), and a repeating `<MismatchedField>` list naming
//! the fields the TR believes do not agree between the two
//! counterparties' submissions.
//!
//! ## Legal note
//!
//! The official SWIFT-licensed XSDs for `auth.106` / `auth.083` are
//! not redistributed with OpenDQI. The adapter parses a plausible,
//! hand-authored structure aligned with public ISO 20022 catalog
//! conventions. If the real schema differs in element names, edit
//! the leaf table in this file accordingly.

use std::path::Path;

use chrono::{DateTime, Utc};
use opendqi_core::{DqDimension, DqIssue, ReconciliationRecord, Regime, Severity};
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::NsReader;

use crate::wellformed::check_wellformedness;

/// Outcome of reading one TR reconciliation XML file.
#[derive(Debug, Default)]
pub struct ReconciliationXmlReadOutcome {
    /// Records extracted from the file.
    pub records: Vec<ReconciliationRecord>,
    /// File-level data-quality issues (format / namespace).
    pub issues: Vec<DqIssue>,
}

const ISO20022_AUTH_106_NS: &[u8] = b"urn:iso:std:iso:20022:tech:xsd:auth.106.001.01";
const ISO20022_AUTH_083_NS: &[u8] = b"urn:iso:std:iso:20022:tech:xsd:auth.083.001.01";

const RCNCLTN_BLOCK: &str = "Rcncltn";

/// Read an EMIR `auth.106` reconciliation file.
pub fn read_emir_reconciliation_xml(path: &Path) -> anyhow::Result<ReconciliationXmlReadOutcome> {
    read_with_regime(path, Regime::Emir, ISO20022_AUTH_106_NS, "auth.106.001.01")
}

/// Read an SFTR `auth.083` reconciliation file.
pub fn read_sftr_reconciliation_xml(path: &Path) -> anyhow::Result<ReconciliationXmlReadOutcome> {
    read_with_regime(path, Regime::Sftr, ISO20022_AUTH_083_NS, "auth.083.001.01")
}

fn read_with_regime(
    path: &Path,
    regime: Regime,
    expected_ns: &[u8],
    expected_label: &str,
) -> anyhow::Result<ReconciliationXmlReadOutcome> {
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
        return Ok(ReconciliationXmlReadOutcome {
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
            Ok(ReconciliationXmlReadOutcome {
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

fn parse(path: &Path, regime: Regime) -> anyhow::Result<ReconciliationXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();
    let mut reader = NsReader::from_file(path)?;
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut pile: Vec<String> = Vec::new();
    let mut is_leaf: Vec<bool> = Vec::new();
    let mut text_buf = String::new();

    let mut header_timestamp: Option<DateTime<Utc>> = None;
    let mut current: Option<ReconciliationRecord> = None;
    let mut rec_depth: Option<usize> = None;
    let mut records: Vec<ReconciliationRecord> = Vec::new();
    let mut rec_index: u32 = 0;

    loop {
        match reader.read_resolved_event_into(&mut buf)? {
            (_, Event::Start(e)) => {
                let local = local_name(&e);
                push_element(&mut pile, &mut is_leaf, local);
                text_buf.clear();

                if current.is_none() && pile.last().map(String::as_str) == Some(RCNCLTN_BLOCK) {
                    rec_index += 1;
                    current = Some(ReconciliationRecord {
                        source_file: Some(source_label.clone()),
                        record_id: Some(format!("{source_label}#rcn-{rec_index}")),
                        regime,
                        reconciliation_timestamp: header_timestamp,
                        ..Default::default()
                    });
                    rec_depth = Some(pile.len());
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
                    if current.is_none()
                        && pile.ends_with(&["Hdr".into(), "RcncltnDtTm".into()])
                        && !trimmed.is_empty()
                    {
                        if let Ok(dt) = DateTime::parse_from_rfc3339(trimmed) {
                            header_timestamp = Some(dt.with_timezone(&Utc));
                        }
                    }
                    if let (Some(rec), Some(rdepth)) = (current.as_mut(), rec_depth) {
                        if pile.len() > rdepth {
                            commit_leaf(rec, &pile[rdepth..], trimmed);
                        }
                    }
                }

                if let Some(rdepth) = rec_depth {
                    if pile.len() == rdepth {
                        if let Some(mut rec) = current.take() {
                            if rec.reconciliation_timestamp.is_none() {
                                rec.reconciliation_timestamp = header_timestamp;
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

    Ok(ReconciliationXmlReadOutcome {
        records,
        issues: Vec::new(),
    })
}

fn commit_leaf(rec: &mut ReconciliationRecord, rel: &[String], value: &str) {
    if value.is_empty() {
        return;
    }
    let path: Vec<&str> = rel.iter().map(String::as_str).collect();
    match path.as_slice() {
        ["UnqTxIdr"] => rec.uti = Some(value.to_owned()),
        ["RptgCtrPty", "LEI"] => rec.reporting_counterparty = Some(value.to_owned()),
        ["OthrCtrPty", "LEI"] => rec.other_counterparty = Some(value.to_owned()),
        ["PrngSts"] => rec.pairing_status = Some(value.to_owned()),
        ["RcncltnSts"] => rec.reconciliation_status = Some(value.to_owned()),
        ["MismatchedField"] => rec.mismatched_fields.push(value.to_owned()),
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
        let p = std::env::temp_dir().join(format!("opendqi-recon-{}-{name}", std::process::id()));
        std::fs::write(&p, content).unwrap();
        p
    }

    #[test]
    fn parses_emir_reconciliation() {
        let body = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.106.001.01">
  <ReconciliationReport>
    <Hdr><RcncltnDtTm>2026-05-13T08:00:00Z</RcncltnDtTm></Hdr>
    <Rcncltn>
      <UnqTxIdr>U1</UnqTxIdr>
      <RptgCtrPty><LEI>RC-LEI</LEI></RptgCtrPty>
      <OthrCtrPty><LEI>OC-LEI</LEI></OthrCtrPty>
      <PrngSts>UNPAIRED</PrngSts>
      <RcncltnSts>UNRECONCILED</RcncltnSts>
    </Rcncltn>
    <Rcncltn>
      <UnqTxIdr>U2</UnqTxIdr>
      <PrngSts>PAIRED</PrngSts>
      <RcncltnSts>UNRECONCILED</RcncltnSts>
      <MismatchedField>NotionalAmount</MismatchedField>
      <MismatchedField>ValuationAmount</MismatchedField>
    </Rcncltn>
  </ReconciliationReport>
</Document>"#;
        let p = write_tmp("emir.xml", body);
        let out = read_emir_reconciliation_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert_eq!(out.records.len(), 2);
        assert_eq!(out.records[0].pairing_status.as_deref(), Some("UNPAIRED"));
        assert_eq!(
            out.records[0].reporting_counterparty.as_deref(),
            Some("RC-LEI")
        );
        assert_eq!(out.records[1].mismatched_fields.len(), 2);
        assert_eq!(out.records[1].mismatched_fields[1], "ValuationAmount");
        assert!(out.records[0].reconciliation_timestamp.is_some());
    }

    #[test]
    fn unsupported_namespace_yields_warning() {
        let body = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.030.001.03"/>"#;
        let p = write_tmp("wrong.xml", body);
        let out = read_emir_reconciliation_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert!(out.records.is_empty());
        assert_eq!(out.issues[0].check_id, "EMIR.FMT.XML_UNSUPPORTED_NAMESPACE");
    }

    #[test]
    fn parses_sftr_reconciliation() {
        let body = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.083.001.01">
  <ReconciliationReport>
    <Rcncltn>
      <UnqTxIdr>S1</UnqTxIdr>
      <PrngSts>UNPAIRED</PrngSts>
    </Rcncltn>
  </ReconciliationReport>
</Document>"#;
        let p = write_tmp("sftr.xml", body);
        let out = read_sftr_reconciliation_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert_eq!(out.records.len(), 1);
        assert_eq!(out.records[0].regime, Regime::Sftr);
    }
}
