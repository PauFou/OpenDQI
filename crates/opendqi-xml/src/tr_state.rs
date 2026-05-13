//! TR Trade State Report (TSR) ingestion — ISO 20022 `auth.107` for
//! EMIR. The official SWIFT-licensed XSD is not redistributed; the
//! adapter parses a plausible structure aligned with public ISO
//! 20022 catalog conventions.
//!
//! See `docs/auth-messages.md` for the canonical message catalog.
//! When the firm has access to the real XSD, the leaf table in
//! [`commit_leaf`] is designed to be edited in one place.

use std::path::Path;
use std::str::FromStr;

use chrono::{DateTime, NaiveDate, Utc};
use opendqi_core::{DqDimension, DqIssue, Regime, Severity, TrStateRecord};
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::NsReader;
use rust_decimal::Decimal;
use tracing::warn;

use crate::wellformed::check_wellformedness;

/// Outcome of reading one TSR XML file.
#[derive(Debug, Default)]
pub struct TrStateXmlReadOutcome {
    /// Records extracted from the file.
    pub records: Vec<TrStateRecord>,
    /// File-level data-quality issues (format / namespace).
    pub issues: Vec<DqIssue>,
}

const ISO20022_AUTH_107_NS: &[u8] = b"urn:iso:std:iso:20022:tech:xsd:auth.107.001.01";
const TRADSTAT_BLOCK: &str = "TradStat";

/// Read an EMIR `auth.107` Trade State Report file.
pub fn read_emir_tr_state_xml(path: &Path) -> anyhow::Result<TrStateXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();

    if let Err(err) = check_wellformedness(path) {
        return Ok(TrStateXmlReadOutcome {
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
            }],
        });
    }

    match peek_root_namespace(path)? {
        Some(ns) if ns == ISO20022_AUTH_107_NS => parse(path),
        other => {
            let actual = other
                .as_deref()
                .map(|n| String::from_utf8_lossy(n).into_owned())
                .unwrap_or_else(|| "(none)".into());
            Ok(TrStateXmlReadOutcome {
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
                        "Root namespace is '{actual}', expected 'urn:iso:std:iso:20022:tech:xsd:auth.107.001.01'."
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

fn parse(path: &Path) -> anyhow::Result<TrStateXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();
    let mut reader = NsReader::from_file(path)?;
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut pile: Vec<String> = Vec::new();
    let mut is_leaf: Vec<bool> = Vec::new();
    let mut text_buf = String::new();
    let mut attrs_buf: Vec<(String, String)> = Vec::new();

    let mut header_state_as_of: Option<DateTime<Utc>> = None;
    let mut current: Option<TrStateRecord> = None;
    let mut rec_depth: Option<usize> = None;
    let mut records: Vec<TrStateRecord> = Vec::new();
    let mut rec_index: u32 = 0;

    loop {
        match reader.read_resolved_event_into(&mut buf)? {
            (_, Event::Start(e)) => {
                let local = local_name(&e);
                push_element(&mut pile, &mut is_leaf, local);
                text_buf.clear();
                attrs_buf = collect_attrs(&e);

                if current.is_none() && pile.last().map(String::as_str) == Some(TRADSTAT_BLOCK) {
                    rec_index += 1;
                    current = Some(TrStateRecord {
                        source_file: Some(source_label.clone()),
                        record_id: Some(format!("{source_label}#tstat-{rec_index}")),
                        regime: Regime::Emir,
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

    Ok(TrStateXmlReadOutcome {
        records,
        issues: Vec::new(),
    })
}

fn commit_leaf(rec: &mut TrStateRecord, rel: &[String], value: &str, attrs: &[(String, String)]) {
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
        ["NotionalAmt"] => {
            set_decimal(&mut rec.notional_amount, value, "NotionalAmt", &record_id);
            if let Some(ccy) = attr_of(attrs, "Ccy") {
                rec.notional_currency = Some(ccy.to_owned());
            }
        }
        ["ValuationAmt"] => {
            set_decimal(&mut rec.valuation_amount, value, "ValuationAmt", &record_id);
            if let Some(ccy) = attr_of(attrs, "Ccy") {
                rec.valuation_currency = Some(ccy.to_owned());
            }
        }
        ["ValuationDtTm"] => {
            set_dt(
                &mut rec.valuation_timestamp,
                value,
                "ValuationDtTm",
                &record_id,
            );
        }
        ["EvntDt"] => set_date(&mut rec.effective_date, value, "EvntDt", &record_id),
        ["MtrtyDt"] => set_date(&mut rec.maturity_date, value, "MtrtyDt", &record_id),
        ["TermntnDt"] => set_date(&mut rec.termination_date, value, "TermntnDt", &record_id),
        ["PrtflCd"] => rec.collateral_portfolio_code = Some(value.to_owned()),
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

fn set_dt(dst: &mut Option<DateTime<Utc>>, s: &str, field: &str, record: &str) {
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
        let p =
            std::env::temp_dir().join(format!("opendqi-tr-state-{}-{name}", std::process::id()));
        std::fs::write(&p, content).unwrap();
        p
    }

    #[test]
    fn parses_two_outstanding_trades() {
        let body = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.107.001.01">
  <TradeStateReport>
    <Hdr><StateAsOf>2026-05-13T08:00:00Z</StateAsOf></Hdr>
    <TradStat>
      <UnqTxIdr>U1</UnqTxIdr>
      <RptgCtrPty><LEI>RC-LEI</LEI></RptgCtrPty>
      <OthrCtrPty><LEI>OC-LEI</LEI></OthrCtrPty>
      <Sts>OUTSTANDING</Sts>
      <NotionalAmt Ccy="EUR">1000000.00</NotionalAmt>
      <ValuationAmt Ccy="EUR">150.50</ValuationAmt>
      <ValuationDtTm>2026-05-13T07:00:00Z</ValuationDtTm>
      <MtrtyDt>2030-01-01</MtrtyDt>
    </TradStat>
    <TradStat>
      <UnqTxIdr>U2</UnqTxIdr>
      <Sts>OUTSTANDING</Sts>
    </TradStat>
  </TradeStateReport>
</Document>"#;
        let p = write_tmp("ok.xml", body);
        let out = read_emir_tr_state_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert_eq!(out.records.len(), 2);
        assert_eq!(out.records[0].uti.as_deref(), Some("U1"));
        assert_eq!(out.records[0].notional_currency.as_deref(), Some("EUR"));
        assert!(out.records[0].state_as_of.is_some());
        assert_eq!(
            out.records[0].valuation_amount.unwrap().to_string(),
            "150.50"
        );
    }

    #[test]
    fn unsupported_namespace_yields_warning() {
        let body = br#"<?xml version="1.0"?><Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.030.001.03"/>"#;
        let p = write_tmp("wrong.xml", body);
        let out = read_emir_tr_state_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert!(out.records.is_empty());
        assert_eq!(out.issues[0].check_id, "EMIR.FMT.XML_UNSUPPORTED_NAMESPACE");
    }

    #[test]
    fn malformed_yields_critical() {
        let body = br#"<?xml version="1.0"?><Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.107.001.01"><TradStat"#;
        let p = write_tmp("bad.xml", body);
        let out = read_emir_tr_state_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert!(out.records.is_empty());
        assert_eq!(out.issues[0].severity, Severity::Critical);
    }
}
