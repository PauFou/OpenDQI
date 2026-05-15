//! TR Trade State Report (TSR) ingestion — ISO 20022 `auth.107` for
//! EMIR.
//!
//! The element paths below are aligned with the real ESMA EMIR REFIT
//! usage guideline `auth.107.001.01_ESMAUG_DATTSR_1.1.0`
//! (`DerivativesTradeStateReportV01`). The SWIFT-licensed XSD is **not**
//! redistributed; only the schema *shape* (element names, nesting,
//! cardinalities) is encoded here. Coverage is intentionally a
//! documented subset — see `docs/auth-messages/emir-auth107.md`.
//!
//! Real envelope:
//! ```text
//! Document
//! └─ DerivsTradStatRpt
//!    ├─ RptHdr/NbRcrds
//!    └─ TradData  (choice)
//!       ├─ DataSetActn = "NOTX"        (empty / no-activity report)
//!       └─ Stat  (1..500000, repeating)
//!          ├─ CtrPtySpcfcData
//!          │  ├─ CtrPty/RptgCtrPty/Id/Lgl/Id/LEI
//!          │  ├─ CtrPty/OthrCtrPty/IdTp/Lgl/Id/LEI
//!          │  ├─ Valtn/CtrctVal/Amt(@Ccy) , Valtn/TmStmp
//!          │  └─ RptgTmStmp             (per-record "state as of")
//!          └─ CmonTradData/TxData
//!             ├─ TxId/UnqTxIdr
//!             ├─ NtnlAmt/FrstLeg/Amt/Amt(@Ccy)
//!             ├─ FctvDt , XprtnDt , EarlyTermntnDt
//!             └─ CollPrtflCd/Prtfl/Cd
//! ```
//! `auth.107` carries no per-trade status element: a record present in
//! a Trade State Report *is* the TR's current outstanding state, so
//! `status` is left `None` (the state-health checks treat `None` as
//! outstanding). The single mapping point is `commit_leaf`.

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
    /// File-level data-quality / parse issues (format / namespace).
    pub issues: Vec<DqIssue>,
}

const ISO20022_AUTH_107_NS: &[u8] = b"urn:iso:std:iso:20022:tech:xsd:auth.107.001.01";
/// The repeating per-trade record element under `TradData`.
const STAT_BLOCK: &str = "Stat";

/// Read an EMIR `auth.107` Trade State Report file.
pub fn read_emir_tr_state_xml(path: &Path) -> anyhow::Result<TrStateXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();

    if let Err(err) = check_wellformedness(path) {
        return Ok(TrStateXmlReadOutcome {
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
        Some(ns) if ns == ISO20022_AUTH_107_NS => parse(path),
        other => {
            let actual = other
                .as_deref()
                .map(|n| String::from_utf8_lossy(n).into_owned())
                .unwrap_or_else(|| "(none)".into());
            Ok(TrStateXmlReadOutcome {
                records: vec![],
                issues: vec![fmt_issue(
                    "EMIR.FMT.XML_UNSUPPORTED_NAMESPACE",
                    Severity::Warning,
                    format!(
                        "Root namespace is '{actual}', expected 'urn:iso:std:iso:20022:tech:xsd:auth.107.001.01'."
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

fn parse(path: &Path) -> anyhow::Result<TrStateXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();
    let mut reader = NsReader::from_file(path)?;
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut pile: Vec<String> = Vec::new();
    let mut is_leaf: Vec<bool> = Vec::new();
    let mut text_buf = String::new();
    let mut attrs_buf: Vec<(String, String)> = Vec::new();

    let mut current: Option<TrStateRecord> = None;
    let mut rec_depth: Option<usize> = None;
    let mut records: Vec<TrStateRecord> = Vec::new();
    let mut rec_index: u32 = 0;
    let mut saw_dataset_actn = false;

    loop {
        match reader.read_resolved_event_into(&mut buf)? {
            (_, Event::Start(e)) => {
                let local = local_name(&e);
                push_element(&mut pile, &mut is_leaf, local);
                text_buf.clear();
                attrs_buf = collect_attrs(&e);

                if current.is_none()
                    && pile.last().map(String::as_str) == Some(STAT_BLOCK)
                    && pile.iter().any(|s| s == "TradData")
                {
                    rec_index += 1;
                    current = Some(TrStateRecord {
                        source_file: Some(source_label.clone()),
                        record_id: Some(format!("{source_label}#stat-{rec_index}")),
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
                    // Empty / no-activity report: TradData/DataSetActn
                    // (value "NOTX") sits *outside* any Stat record.
                    if current.is_none()
                        && pile.ends_with(&["TradData".into(), "DataSetActn".into()])
                    {
                        saw_dataset_actn = true;
                    }
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

    let mut issues = Vec::new();
    if records.is_empty() && saw_dataset_actn {
        issues.push(fmt_issue(
            "EMIR.FMT.TSR_NO_RECORDS",
            Severity::Info,
            "Trade State Report carries TradData/DataSetActn (no-activity \
             report); zero state records to evaluate."
                .to_string(),
            source_label,
        ));
    }

    Ok(TrStateXmlReadOutcome { records, issues })
}

/// True when `rel` ends with `suffix` (element-name tail match).
fn tail(rel: &[String], suffix: &[&str]) -> bool {
    rel.len() >= suffix.len()
        && rel[rel.len() - suffix.len()..]
            .iter()
            .zip(suffix)
            .all(|(a, b)| a == *b)
}

/// True when `seg` appears anywhere in `rel` (ancestor disambiguation).
fn has(rel: &[String], seg: &str) -> bool {
    rel.iter().any(|s| s == seg)
}

/// Map one leaf (path relative to the `Stat` record) onto the canonical
/// `TrStateRecord`. Real `auth.107.001.01` element paths; the entire
/// `TradeTransaction49` subtree beyond the fields below is intentionally
/// not extracted (documented in `docs/auth-messages/emir-auth107.md`).
fn commit_leaf(rec: &mut TrStateRecord, rel: &[String], value: &str, attrs: &[(String, String)]) {
    if value.is_empty() && attrs.is_empty() {
        return;
    }
    let record_id = rec.record_id.clone().unwrap_or_default();

    // UTI: TxId/UnqTxIdr (or proprietary TxId/Prtry/Id).
    if tail(rel, &["TxId", "UnqTxIdr"]) || tail(rel, &["TxId", "Prtry", "Id"]) {
        if rec.uti.is_none() && !value.is_empty() {
            rec.uti = Some(value.to_owned());
        }
        return;
    }
    // Counterparty LEIs: …/RptgCtrPty/…/LEI and …/OthrCtrPty/…/LEI.
    if tail(rel, &["LEI"]) && has(rel, "RptgCtrPty") {
        rec.reporting_counterparty = Some(value.to_owned());
        return;
    }
    if tail(rel, &["LEI"]) && has(rel, "OthrCtrPty") {
        rec.other_counterparty = Some(value.to_owned());
        return;
    }
    // Valuation: CtrPtySpcfcData/Valtn/CtrctVal/Amt(@Ccy) + TmStmp.
    if tail(rel, &["Valtn", "CtrctVal", "Amt"]) {
        set_decimal(
            &mut rec.valuation_amount,
            value,
            "Valtn/CtrctVal/Amt",
            &record_id,
        );
        if let Some(ccy) = attr_of(attrs, "Ccy") {
            rec.valuation_currency = Some(ccy.to_owned());
        }
        return;
    }
    if tail(rel, &["Valtn", "TmStmp"]) {
        set_dt(
            &mut rec.valuation_timestamp,
            value,
            "Valtn/TmStmp",
            &record_id,
        );
        return;
    }
    // "State as of": per-record CtrPtySpcfcData/RptgTmStmp (no header
    // StateAsOf exists in the real schema).
    if rel.last().map(String::as_str) == Some("RptgTmStmp") {
        let mut tmp = None;
        set_dt(&mut tmp, value, "RptgTmStmp", &record_id);
        if tmp.is_some() {
            rec.state_as_of = tmp;
        }
        return;
    }
    // Notional: TxData/NtnlAmt/FrstLeg/Amt/Amt(@Ccy) — first leg only.
    if tail(rel, &["NtnlAmt", "FrstLeg", "Amt", "Amt"]) {
        set_decimal(
            &mut rec.notional_amount,
            value,
            "NtnlAmt/FrstLeg/Amt/Amt",
            &record_id,
        );
        if let Some(ccy) = attr_of(attrs, "Ccy") {
            rec.notional_currency = Some(ccy.to_owned());
        }
        return;
    }
    // Dates on TxData.
    match rel.last().map(String::as_str) {
        Some("FctvDt") => {
            set_date(&mut rec.effective_date, value, "FctvDt", &record_id);
            return;
        }
        Some("XprtnDt") => {
            set_date(&mut rec.maturity_date, value, "XprtnDt", &record_id);
            return;
        }
        Some("EarlyTermntnDt") => {
            set_date(
                &mut rec.termination_date,
                value,
                "EarlyTermntnDt",
                &record_id,
            );
            return;
        }
        _ => {}
    }
    // Collateral portfolio code: CollPrtflCd/Prtfl/Cd ("NoPrtfl" → none).
    if tail(rel, &["CollPrtflCd", "Prtfl", "Cd"]) {
        rec.collateral_portfolio_code = Some(value.to_owned());
        return;
    }
    if tail(rel, &["CollPrtflCd", "Prtfl", "NoPrtfl"]) {
        return;
    }
    // Everything else in the real schema is preserved verbatim.
    if !value.is_empty() {
        rec.raw_fields.insert(rel.join("/"), value.to_owned());
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

    /// One full schema-shaped `Stat` record using the real
    /// `auth.107.001.01` element paths.
    const REAL_ENVELOPE: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.107.001.01">
  <DerivsTradStatRpt>
    <RptHdr><NbRcrds>2</NbRcrds></RptHdr>
    <TradData>
      <Stat>
        <CtrPtySpcfcData>
          <CtrPty>
            <RptgCtrPty><Id><Lgl><Id><LEI>RPTGCPARTY0000000001</LEI></Id></Lgl></Id></RptgCtrPty>
            <OthrCtrPty><IdTp><Lgl><Id><LEI>OTHRCPARTY0000000002</LEI></Id></Lgl></IdTp></OthrCtrPty>
          </CtrPty>
          <Valtn>
            <CtrctVal><Amt Ccy="EUR">150.50</Amt></CtrctVal>
            <TmStmp>2026-05-13T07:00:00Z</TmStmp>
          </Valtn>
          <RptgTmStmp>2026-05-13T08:00:00Z</RptgTmStmp>
        </CtrPtySpcfcData>
        <CmonTradData>
          <TxData>
            <TxId><UnqTxIdr>UTICLEAN0000000000001</UnqTxIdr></TxId>
            <NtnlAmt><FrstLeg><Amt><Amt Ccy="EUR">1000000.00</Amt></Amt></FrstLeg></NtnlAmt>
            <FctvDt>2024-01-01</FctvDt>
            <XprtnDt>2030-01-01</XprtnDt>
            <CollPrtflCd><Prtfl><Cd>PF-1</Cd></Prtfl></CollPrtflCd>
          </TxData>
        </CmonTradData>
      </Stat>
      <Stat>
        <CtrPtySpcfcData>
          <RptgTmStmp>2026-05-13T08:00:00Z</RptgTmStmp>
        </CtrPtySpcfcData>
        <CmonTradData>
          <TxData>
            <TxId><UnqTxIdr>UTICLEAN0000000000002</UnqTxIdr></TxId>
          </TxData>
        </CmonTradData>
      </Stat>
    </TradData>
  </DerivsTradStatRpt>
</Document>"#;

    #[test]
    fn parses_real_auth107_envelope() {
        let p = write_tmp("real.xml", REAL_ENVELOPE);
        let out = read_emir_tr_state_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert!(out.issues.is_empty());
        assert_eq!(out.records.len(), 2);
        let r0 = &out.records[0];
        assert_eq!(r0.uti.as_deref(), Some("UTICLEAN0000000000001"));
        assert_eq!(
            r0.reporting_counterparty.as_deref(),
            Some("RPTGCPARTY0000000001")
        );
        assert_eq!(
            r0.other_counterparty.as_deref(),
            Some("OTHRCPARTY0000000002")
        );
        assert_eq!(r0.notional_currency.as_deref(), Some("EUR"));
        assert_eq!(r0.notional_amount.unwrap().to_string(), "1000000.00");
        assert_eq!(r0.valuation_currency.as_deref(), Some("EUR"));
        assert_eq!(r0.valuation_amount.unwrap().to_string(), "150.50");
        assert_eq!(r0.collateral_portfolio_code.as_deref(), Some("PF-1"));
        // status carries no schema element → None (= outstanding).
        assert!(r0.status.is_none());
        // "state as of" comes from the per-record RptgTmStmp.
        assert!(r0.state_as_of.is_some());
        assert!(r0.valuation_timestamp.is_some());
        assert_eq!(r0.maturity_date.unwrap().to_string(), "2030-01-01");
    }

    #[test]
    fn state_as_of_from_reporting_timestamp() {
        let p = write_tmp("sao.xml", REAL_ENVELOPE);
        let out = read_emir_tr_state_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        let sao = out.records[0].state_as_of.unwrap();
        assert_eq!(sao.to_rfc3339(), "2026-05-13T08:00:00+00:00");
    }

    #[test]
    fn empty_report_no_records_info() {
        let body = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.107.001.01">
  <DerivsTradStatRpt>
    <RptHdr><NbRcrds>0</NbRcrds></RptHdr>
    <TradData><DataSetActn>NOTX</DataSetActn></TradData>
  </DerivsTradStatRpt>
</Document>"#;
        let p = write_tmp("empty.xml", body);
        let out = read_emir_tr_state_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert!(out.records.is_empty());
        assert_eq!(out.issues.len(), 1);
        assert_eq!(out.issues[0].check_id, "EMIR.FMT.TSR_NO_RECORDS");
        assert_eq!(out.issues[0].severity, Severity::Info);
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
        let body = br#"<?xml version="1.0"?><Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.107.001.01"><Stat"#;
        let p = write_tmp("bad.xml", body);
        let out = read_emir_tr_state_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert!(out.records.is_empty());
        assert_eq!(out.issues[0].severity, Severity::Critical);
    }
}
