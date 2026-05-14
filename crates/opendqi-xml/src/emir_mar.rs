//! EMIR Margin Activity Report (MAR) ingestion — ISO 20022 `auth.108`.
//! Streaming `NsReader` adapter; synthetic plausible structure.
//! Adapt the leaf table below once the official SWIFT-licensed XSD is
//! available.

use std::path::Path;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use opendqi_core::{DqDimension, DqIssue, MarginActivityRecord, Regime, Severity};
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::NsReader;
use rust_decimal::Decimal;
use tracing::warn;

use crate::wellformed::check_wellformedness;

/// Outcome of reading one MAR XML file.
#[derive(Debug, Default)]
pub struct MarXmlReadOutcome {
    /// Records extracted from the file.
    pub records: Vec<MarginActivityRecord>,
    /// File-level data-quality issues (format / namespace).
    pub issues: Vec<DqIssue>,
}

const ISO20022_AUTH_108_NS: &[u8] = b"urn:iso:std:iso:20022:tech:xsd:auth.108.001.01";
const MARGIN_BLOCK: &str = "MrgnEvt";

/// Read an EMIR `auth.108` Margin Activity Report file.
pub fn read_emir_mar_xml(path: &Path) -> anyhow::Result<MarXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();

    if let Err(err) = check_wellformedness(path) {
        return Ok(MarXmlReadOutcome {
            records: vec![],
            issues: vec![DqIssue {
                check_id: "EMIR.FMT.XML_NOT_WELLFORMED".into(),
                regime: Regime::Emir,
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
        Some(ns) if ns == ISO20022_AUTH_108_NS => parse(path),
        other => {
            let actual = other
                .as_deref()
                .map(|n| String::from_utf8_lossy(n).into_owned())
                .unwrap_or_else(|| "(none)".into());
            Ok(MarXmlReadOutcome {
                records: vec![],
                issues: vec![DqIssue {
                    check_id: "EMIR.FMT.XML_UNSUPPORTED_NAMESPACE".into(),
                    regime: Regime::Emir,
                    severity: Severity::Warning,
                    dimension: DqDimension::Validity,
                    record_id: None,
                    uti: None,
                    field: None,
                    value: None,
                    message: format!(
                        "Root namespace is '{actual}', expected 'urn:iso:std:iso:20022:tech:xsd:auth.108.001.01'."
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

fn parse(path: &Path) -> anyhow::Result<MarXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();
    let mut reader = NsReader::from_file(path)?;
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut pile: Vec<String> = Vec::new();
    let mut is_leaf: Vec<bool> = Vec::new();
    let mut text_buf = String::new();
    let mut attrs_buf: Vec<(String, String)> = Vec::new();

    let mut current: Option<MarginActivityRecord> = None;
    let mut rec_depth: Option<usize> = None;
    let mut records: Vec<MarginActivityRecord> = Vec::new();
    let mut rec_index: u32 = 0;

    loop {
        match reader.read_resolved_event_into(&mut buf)? {
            (_, Event::Start(e)) => {
                let local = local_name(&e);
                push_element(&mut pile, &mut is_leaf, local);
                text_buf.clear();
                attrs_buf = collect_attrs(&e);

                if current.is_none() && pile.last().map(String::as_str) == Some(MARGIN_BLOCK) {
                    rec_index += 1;
                    current = Some(MarginActivityRecord {
                        source_file: Some(source_label.clone()),
                        record_id: Some(format!("{source_label}#mrgnevt-{rec_index}")),
                        regime: Regime::Emir,
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
                attrs_buf.clear();
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
                    if let (Some(rec), Some(rdepth)) = (current.as_mut(), rec_depth) {
                        if pile.len() > rdepth {
                            commit_leaf(rec, &pile[rdepth..], trimmed, &attrs_buf);
                        }
                    }
                }

                if let Some(rdepth) = rec_depth {
                    if pile.len() == rdepth {
                        if let Some(rec) = current.take() {
                            records.push(rec);
                        }
                        rec_depth = None;
                    }
                }

                pop_element(&mut pile, &mut is_leaf);
                text_buf.clear();
                attrs_buf.clear();
            }
            (_, Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(MarXmlReadOutcome {
        records,
        issues: Vec::new(),
    })
}

fn commit_leaf(
    rec: &mut MarginActivityRecord,
    rel: &[String],
    value: &str,
    attrs: &[(String, String)],
) {
    if value.is_empty() && attrs.is_empty() {
        return;
    }
    let path: Vec<&str> = rel.iter().map(String::as_str).collect();
    let record_id = rec.record_id.clone().unwrap_or_default();
    match path.as_slice() {
        ["UnqTxIdr"] => rec.uti = Some(value.to_owned()),
        ["CtrPty1", "LEI"] => rec.counterparty_1 = Some(value.to_owned()),
        ["CtrPty2", "LEI"] => rec.counterparty_2 = Some(value.to_owned()),
        ["ActnTp"] => rec.action_type = Some(value.to_owned()),
        ["EvntTp"] => rec.event_type = Some(value.to_owned()),
        ["PrtflCd"] => rec.collateral_portfolio_code = Some(value.to_owned()),
        ["IMPstd"] => {
            set_decimal(&mut rec.initial_margin_posted, value, "IMPstd", &record_id);
            if let Some(ccy) = attr_of(attrs, "Ccy") {
                rec.margin_currency = Some(ccy.to_owned());
            }
        }
        ["IMColl"] => {
            set_decimal(
                &mut rec.initial_margin_collected,
                value,
                "IMColl",
                &record_id,
            );
            if let Some(ccy) = attr_of(attrs, "Ccy") {
                rec.margin_currency = Some(ccy.to_owned());
            }
        }
        ["VMPstd"] => {
            set_decimal(
                &mut rec.variation_margin_posted,
                value,
                "VMPstd",
                &record_id,
            );
            if let Some(ccy) = attr_of(attrs, "Ccy") {
                rec.margin_currency = Some(ccy.to_owned());
            }
        }
        ["VMColl"] => {
            set_decimal(
                &mut rec.variation_margin_collected,
                value,
                "VMColl",
                &record_id,
            );
            if let Some(ccy) = attr_of(attrs, "Ccy") {
                rec.margin_currency = Some(ccy.to_owned());
            }
        }
        ["XcssColl"] => set_decimal(&mut rec.excess_collateral, value, "XcssColl", &record_id),
        ["Hrcut"] => set_decimal(&mut rec.collateral_haircut, value, "Hrcut", &record_id),
        ["EvntDtTm"] => set_datetime(&mut rec.event_timestamp, value, "EvntDtTm", &record_id),
        ["RptgDtTm"] => set_datetime(&mut rec.reporting_timestamp, value, "RptgDtTm", &record_id),
        _ => {
            let key = rel.join("/");
            if !value.is_empty() {
                rec.raw_fields.insert(key, value.to_owned());
            }
        }
    }
}

fn attr_of<'a>(attrs: &'a [(String, String)], key: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
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

fn collect_attrs(e: &BytesStart<'_>) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for attr in e.attributes().flatten() {
        let key = String::from_utf8_lossy(attr.key.local_name().as_ref()).into_owned();
        let value = attr
            .unescape_value()
            .map(|c| c.into_owned())
            .unwrap_or_default();
        out.push((key, value));
    }
    out
}

fn set_decimal(dst: &mut Option<Decimal>, s: &str, field: &str, record: &str) {
    if s.is_empty() {
        return;
    }
    match Decimal::from_str(s) {
        Ok(d) => *dst = Some(d),
        Err(e) => warn!(record = %record, field, value = s, error = %e, "could not parse decimal"),
    }
}

fn set_datetime(dst: &mut Option<DateTime<Utc>>, s: &str, field: &str, record: &str) {
    if s.is_empty() {
        return;
    }
    match DateTime::parse_from_rfc3339(s) {
        Ok(d) => *dst = Some(d.with_timezone(&Utc)),
        Err(e) => warn!(record = %record, field, value = s, error = %e, "could not parse datetime"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_tmp(name: &str, content: &[u8]) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("opendqi-mar-{}-{name}", std::process::id()));
        std::fs::write(&p, content).unwrap();
        p
    }

    #[test]
    fn parses_two_margin_events() {
        let body = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.108.001.01">
  <MrgnActvtyRpt>
    <MrgnEvt>
      <UnqTxIdr>U1</UnqTxIdr>
      <CtrPty1><LEI>LEI-A</LEI></CtrPty1>
      <CtrPty2><LEI>LEI-B</LEI></CtrPty2>
      <ActnTp>MARU</ActnTp>
      <PrtflCd>P1</PrtflCd>
      <IMPstd Ccy="EUR">1000000</IMPstd>
      <VMPstd Ccy="EUR">50000</VMPstd>
      <EvntDtTm>2026-05-12T16:00:00Z</EvntDtTm>
      <RptgDtTm>2026-05-13T08:30:00Z</RptgDtTm>
    </MrgnEvt>
    <MrgnEvt>
      <UnqTxIdr>U2</UnqTxIdr>
      <ActnTp>MARV</ActnTp>
      <PrtflCd>P2</PrtflCd>
      <VMColl Ccy="USD">25000</VMColl>
    </MrgnEvt>
  </MrgnActvtyRpt>
</Document>"#;
        let p = write_tmp("ok.xml", body);
        let out = read_emir_mar_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert_eq!(out.records.len(), 2);
        assert_eq!(out.records[0].uti.as_deref(), Some("U1"));
        assert_eq!(out.records[0].action_type.as_deref(), Some("MARU"));
        assert_eq!(out.records[0].margin_currency.as_deref(), Some("EUR"));
        assert!(out.records[0].initial_margin_posted.is_some());
        assert!(out.records[0].event_timestamp.is_some());
        assert_eq!(out.records[1].action_type.as_deref(), Some("MARV"));
        assert_eq!(out.records[1].margin_currency.as_deref(), Some("USD"));
    }

    #[test]
    fn unsupported_namespace_yields_warning() {
        let body = br#"<?xml version="1.0"?><Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.030.001.04"/>"#;
        let p = write_tmp("wrong.xml", body);
        let out = read_emir_mar_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert!(out.records.is_empty());
        assert_eq!(out.issues[0].check_id, "EMIR.FMT.XML_UNSUPPORTED_NAMESPACE");
    }
}
