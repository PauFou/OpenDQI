//! SFTR Trade State Report (TSR) ingestion — ISO 20022 `auth.079`.
//! Streaming `NsReader` adapter, plausible synthetic structure;
//! adapt the leaf table below once the official SWIFT-licensed XSD
//! is available.

use std::path::Path;
use std::str::FromStr;

use chrono::{DateTime, NaiveDate, Utc};
use opendqi_core::{DqDimension, DqIssue, Regime, Severity, SftrTrStateRecord};
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::NsReader;
use rust_decimal::Decimal;
use tracing::warn;

use crate::wellformed::check_wellformedness;

/// Outcome of reading one SFTR TSR XML file.
#[derive(Debug, Default)]
pub struct SftrTrStateXmlReadOutcome {
    /// Records extracted from the file.
    pub records: Vec<SftrTrStateRecord>,
    /// File-level data-quality issues (format / namespace).
    pub issues: Vec<DqIssue>,
}

const ISO20022_AUTH_079_NS: &[u8] = b"urn:iso:std:iso:20022:tech:xsd:auth.079.001.01";
const SFT_STAT_BLOCK: &str = "SftStat";

/// Read an SFTR `auth.079` Trade State Report file.
pub fn read_sftr_tr_state_xml(path: &Path) -> anyhow::Result<SftrTrStateXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();

    if let Err(err) = check_wellformedness(path) {
        return Ok(SftrTrStateXmlReadOutcome {
            records: vec![],
            issues: vec![DqIssue {
                check_id: "SFTR.FMT.XML_NOT_WELLFORMED".into(),
                regime: Regime::Sftr,
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
        Some(ns) if ns == ISO20022_AUTH_079_NS => parse(path),
        other => {
            let actual = other
                .as_deref()
                .map(|n| String::from_utf8_lossy(n).into_owned())
                .unwrap_or_else(|| "(none)".into());
            Ok(SftrTrStateXmlReadOutcome {
                records: vec![],
                issues: vec![DqIssue {
                    check_id: "SFTR.FMT.XML_UNSUPPORTED_NAMESPACE".into(),
                    regime: Regime::Sftr,
                    severity: Severity::Warning,
                    dimension: DqDimension::Validity,
                    record_id: None,
                    uti: None,
                    field: None,
                    value: None,
                    message: format!(
                        "Root namespace is '{actual}', expected 'urn:iso:std:iso:20022:tech:xsd:auth.079.001.01'."
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

fn parse(path: &Path) -> anyhow::Result<SftrTrStateXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();
    let mut reader = NsReader::from_file(path)?;
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut pile: Vec<String> = Vec::new();
    let mut is_leaf: Vec<bool> = Vec::new();
    let mut text_buf = String::new();
    let mut attrs_buf: Vec<(String, String)> = Vec::new();

    let mut header_state_as_of: Option<DateTime<Utc>> = None;
    let mut current: Option<SftrTrStateRecord> = None;
    let mut rec_depth: Option<usize> = None;
    let mut records: Vec<SftrTrStateRecord> = Vec::new();
    let mut rec_index: u32 = 0;

    loop {
        match reader.read_resolved_event_into(&mut buf)? {
            (_, Event::Start(e)) => {
                let local = local_name(&e);
                push_element(&mut pile, &mut is_leaf, local);
                text_buf.clear();
                attrs_buf = collect_attrs(&e);

                if current.is_none() && pile.last().map(String::as_str) == Some(SFT_STAT_BLOCK) {
                    rec_index += 1;
                    current = Some(SftrTrStateRecord {
                        source_file: Some(source_label.clone()),
                        record_id: Some(format!("{source_label}#sftstat-{rec_index}")),
                        regime: Regime::Sftr,
                        state_as_of: header_state_as_of,
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
                    if current.is_none()
                        && pile.ends_with(&["Hdr".into(), "StateAsOf".into()])
                        && !trimmed.is_empty()
                    {
                        if let Ok(dt) = DateTime::parse_from_rfc3339(trimmed) {
                            header_state_as_of = Some(dt.with_timezone(&Utc));
                        }
                    }
                    if let (Some(rec), Some(rdepth)) = (current.as_mut(), rec_depth) {
                        if pile.len() > rdepth {
                            commit_leaf(rec, &pile[rdepth..], trimmed, &attrs_buf);
                        }
                    }
                }

                if let Some(rdepth) = rec_depth {
                    if pile.len() == rdepth {
                        if let Some(mut rec) = current.take() {
                            if rec.state_as_of.is_none() {
                                rec.state_as_of = header_state_as_of;
                            }
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

    Ok(SftrTrStateXmlReadOutcome {
        records,
        issues: Vec::new(),
    })
}

fn commit_leaf(
    rec: &mut SftrTrStateRecord,
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
        ["RptgCtrPty", "LEI"] => rec.reporting_counterparty = Some(value.to_owned()),
        ["OthrCtrPty", "LEI"] => rec.other_counterparty = Some(value.to_owned()),
        ["Sts"] => rec.status = Some(value.to_owned()),
        ["SftTp"] => rec.sft_type = Some(value.to_owned()),
        ["LoanAmt"] => {
            set_decimal(&mut rec.loan_value, value, "LoanAmt", &record_id);
            if let Some(ccy) = attr_of(attrs, "Ccy") {
                rec.loan_currency = Some(ccy.to_owned());
            }
        }
        ["CollVal"] => {
            set_decimal(&mut rec.collateral_value, value, "CollVal", &record_id);
            if let Some(ccy) = attr_of(attrs, "Ccy") {
                rec.collateral_currency = Some(ccy.to_owned());
            }
        }
        ["Hrcut"] => set_decimal(&mut rec.haircut, value, "Hrcut", &record_id),
        ["RuseInd"] => {
            rec.reuse_indicator = match value.to_ascii_lowercase().as_str() {
                "true" | "1" | "y" | "yes" => Some(true),
                "false" | "0" | "n" | "no" => Some(false),
                _ => rec.reuse_indicator,
            };
        }
        ["EvntDt"] => set_date(&mut rec.effective_date, value, "EvntDt", &record_id),
        ["MtrtyDt"] => set_date(&mut rec.maturity_date, value, "MtrtyDt", &record_id),
        ["TermntnDt"] => set_date(&mut rec.termination_date, value, "TermntnDt", &record_id),
        ["SttlmDt"] => set_date(&mut rec.settlement_date, value, "SttlmDt", &record_id),
        ["PrtflCd"] => rec.collateral_portfolio_code = Some(value.to_owned()),
        ["ISIN"] | ["Sctys", "ISIN"] | ["Sctys", "Id"] => {
            rec.collateral_isin = Some(value.to_owned())
        }
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

fn set_date(dst: &mut Option<NaiveDate>, s: &str, field: &str, record: &str) {
    if s.is_empty() {
        return;
    }
    match NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        Ok(d) => *dst = Some(d),
        Err(e) => warn!(record = %record, field, value = s, error = %e, "could not parse date"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_tmp(name: &str, content: &[u8]) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!(
            "opendqi-sftr-tr-state-{}-{name}",
            std::process::id()
        ));
        std::fs::write(&p, content).unwrap();
        p
    }

    #[test]
    fn parses_two_outstanding_sfts() {
        let body = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.079.001.01">
  <SftStateReport>
    <Hdr><StateAsOf>2026-05-13T08:00:00Z</StateAsOf></Hdr>
    <SftStat>
      <UnqTxIdr>S1</UnqTxIdr>
      <RptgCtrPty><LEI>RC-LEI</LEI></RptgCtrPty>
      <OthrCtrPty><LEI>OC-LEI</LEI></OthrCtrPty>
      <Sts>OUTSTANDING</Sts>
      <SftTp>REPO</SftTp>
      <LoanAmt Ccy="EUR">1000000</LoanAmt>
      <CollVal Ccy="EUR">1100000</CollVal>
      <Hrcut>0.05</Hrcut>
      <MtrtyDt>2030-01-01</MtrtyDt>
      <RuseInd>true</RuseInd>
    </SftStat>
    <SftStat>
      <UnqTxIdr>S2</UnqTxIdr>
      <Sts>OUTSTANDING</Sts>
    </SftStat>
  </SftStateReport>
</Document>"#;
        let p = write_tmp("ok.xml", body);
        let out = read_sftr_tr_state_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert_eq!(out.records.len(), 2);
        assert_eq!(out.records[0].uti.as_deref(), Some("S1"));
        assert_eq!(out.records[0].sft_type.as_deref(), Some("REPO"));
        assert_eq!(out.records[0].loan_currency.as_deref(), Some("EUR"));
        assert_eq!(out.records[0].reuse_indicator, Some(true));
        assert!(out.records[0].state_as_of.is_some());
    }

    #[test]
    fn unsupported_namespace_yields_warning() {
        let body = br#"<?xml version="1.0"?><Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.052.001.02"/>"#;
        let p = write_tmp("wrong.xml", body);
        let out = read_sftr_tr_state_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert!(out.records.is_empty());
        assert_eq!(out.issues[0].check_id, "SFTR.FMT.XML_UNSUPPORTED_NAMESPACE");
    }
}
