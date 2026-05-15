//! EMIR Margin Activity Report (MAR) ingestion — ISO 20022 `auth.108`.
//!
//! Element paths are aligned with the real ESMA EMIR REFIT usage
//! guideline `auth.108.001.01_ESMAUG_DATMDA_1.1.0`
//! (`DerivativesTradeMarginDataReportV01`). The SWIFT-licensed XSD is
//! **not** redistributed; only the schema *shape* (element names,
//! nesting, cardinalities) is encoded here. Coverage is intentionally
//! a documented subset — see `docs/auth-messages/emir-auth108.md`.
//!
//! Real envelope:
//! ```text
//! Document
//! └─ DerivsTradMrgnDataRpt          (DerivativesTradeMarginDataReportV01)
//!    ├─ RptHdr/NbRcrds
//!    └─ TradData  (choice)
//!       ├─ DataSetActn = "NOTX"     (empty / no-activity report)
//!       └─ Rpt  (1..500000)         (TradeReport31Choice)
//!          └─ MrgnUpd | Crrctn      (MarginReportData7 — wrapper = action)
//!             ├─ RptgTmStmp , EvtDt
//!             ├─ CtrPtyId/RptgCtrPty/Id/Lgl/Id/LEI
//!             ├─ CtrPtyId/OthrCtrPty/IdTp/Lgl/Id/LEI
//!             ├─ TxId/UnqTxIdr
//!             ├─ Coll/CollPrtflCd/Prtfl/Cd , Coll/CollstnCtgy
//!             ├─ PstdMrgnOrColl/{InitlMrgnPstdPstHrcut,
//!             │                  VartnMrgnPstdPstHrcut, XcssCollPstd}(@Ccy)
//!             └─ RcvdMrgnOrColl/{InitlMrgnRcvdPstHrcut,
//!                                VartnMrgnRcvdPstHrcut, XcssCollRcvd}(@Ccy)
//! ```
//! "Collected" maps to the schema's *received* side; the post-haircut
//! amounts are taken as the canonical economic values (pre-haircut →
//! `raw_fields`). `auth.108` carries no haircut percentage and no
//! event datetime (only an `EvtDt` date, normalised to `T00:00:00Z`).
//! The single mapping point is `commit_leaf`.

use std::path::Path;
use std::str::FromStr;

use chrono::{DateTime, NaiveDate, Utc};
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
    /// File-level data-quality / parse issues (format / namespace).
    pub issues: Vec<DqIssue>,
}

const ISO20022_AUTH_108_NS: &[u8] = b"urn:iso:std:iso:20022:tech:xsd:auth.108.001.01";
/// The repeating per-trade record element under `TradData`.
const RPT_BLOCK: &str = "Rpt";

/// Read an EMIR `auth.108` Margin Activity Report file.
pub fn read_emir_mar_xml(path: &Path) -> anyhow::Result<MarXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();

    if let Err(err) = check_wellformedness(path) {
        return Ok(MarXmlReadOutcome {
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
        Some(ns) if ns == ISO20022_AUTH_108_NS => parse(path),
        other => {
            let actual = other
                .as_deref()
                .map(|n| String::from_utf8_lossy(n).into_owned())
                .unwrap_or_else(|| "(none)".into());
            Ok(MarXmlReadOutcome {
                records: vec![],
                issues: vec![fmt_issue(
                    "EMIR.FMT.XML_UNSUPPORTED_NAMESPACE",
                    Severity::Warning,
                    format!(
                        "Root namespace is '{actual}', expected 'urn:iso:std:iso:20022:tech:xsd:auth.108.001.01'."
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
    let mut saw_dataset_actn = false;

    loop {
        match reader.read_resolved_event_into(&mut buf)? {
            (_, Event::Start(e)) => {
                let local = local_name(&e);
                push_element(&mut pile, &mut is_leaf, local);
                text_buf.clear();
                attrs_buf = collect_attrs(&e);

                if current.is_none()
                    && pile.last().map(String::as_str) == Some(RPT_BLOCK)
                    && pile.iter().any(|s| s == "TradData")
                {
                    rec_index += 1;
                    current = Some(MarginActivityRecord {
                        source_file: Some(source_label.clone()),
                        record_id: Some(format!("{source_label}#rpt-{rec_index}")),
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
            "EMIR.FMT.MAR_NO_RECORDS",
            Severity::Info,
            "Margin Activity Report carries TradData/DataSetActn \
             (no-activity report); zero margin records to evaluate."
                .to_string(),
            source_label,
        ));
    }

    Ok(MarXmlReadOutcome { records, issues })
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

/// Map one leaf (path relative to the `Rpt` record) onto the canonical
/// `MarginActivityRecord`. Real `auth.108.001.01` element paths; every
/// other branch is intentionally not extracted (documented in
/// `docs/auth-messages/emir-auth108.md`).
fn commit_leaf(
    rec: &mut MarginActivityRecord,
    rel: &[String],
    value: &str,
    attrs: &[(String, String)],
) {
    if value.is_empty() && attrs.is_empty() {
        return;
    }
    let record_id = rec.record_id.clone().unwrap_or_default();

    // Action type from the TradeReport31Choice wrapper (no free ActnTp
    // element in auth.108): Crrctn → CORR, MrgnUpd → MRGN.
    if rec.action_type.is_none() {
        if has(rel, "Crrctn") {
            rec.action_type = Some("CORR".to_owned());
        } else if has(rel, "MrgnUpd") {
            rec.action_type = Some("MRGN".to_owned());
        }
    }

    if tail(rel, &["TxId", "UnqTxIdr"]) || tail(rel, &["TxId", "Prtry", "Id"]) {
        if rec.uti.is_none() && !value.is_empty() {
            rec.uti = Some(value.to_owned());
        }
        return;
    }
    if tail(rel, &["LEI"]) && has(rel, "RptgCtrPty") {
        rec.counterparty_1 = Some(value.to_owned());
        return;
    }
    if tail(rel, &["LEI"]) && has(rel, "OthrCtrPty") {
        rec.counterparty_2 = Some(value.to_owned());
        return;
    }
    if rel.last().map(String::as_str) == Some("RptgTmStmp") {
        set_dt(
            &mut rec.reporting_timestamp,
            value,
            "RptgTmStmp",
            &record_id,
        );
        return;
    }
    if rel.last().map(String::as_str) == Some("EvtDt") {
        // auth.108 carries an event *date*, not a datetime; normalise
        // to midnight UTC so EMIR.MAR.TIMELINESS stays functional.
        set_date_as_dt(&mut rec.event_timestamp, value, "EvtDt", &record_id);
        return;
    }
    if tail(rel, &["CollPrtflCd", "Prtfl", "Cd"]) {
        rec.collateral_portfolio_code = Some(value.to_owned());
        return;
    }
    if tail(rel, &["CollPrtflCd", "Prtfl", "NoPrtfl"]) {
        return;
    }
    if tail(rel, &["InitlMrgnPstdPstHrcut"]) {
        set_decimal(
            &mut rec.initial_margin_posted,
            value,
            "InitlMrgnPstdPstHrcut",
            &record_id,
        );
        set_ccy(&mut rec.margin_currency, attrs);
        return;
    }
    if tail(rel, &["InitlMrgnRcvdPstHrcut"]) {
        set_decimal(
            &mut rec.initial_margin_collected,
            value,
            "InitlMrgnRcvdPstHrcut",
            &record_id,
        );
        set_ccy(&mut rec.margin_currency, attrs);
        return;
    }
    if tail(rel, &["VartnMrgnPstdPstHrcut"]) {
        set_decimal(
            &mut rec.variation_margin_posted,
            value,
            "VartnMrgnPstdPstHrcut",
            &record_id,
        );
        set_ccy(&mut rec.margin_currency, attrs);
        return;
    }
    if tail(rel, &["VartnMrgnRcvdPstHrcut"]) {
        set_decimal(
            &mut rec.variation_margin_collected,
            value,
            "VartnMrgnRcvdPstHrcut",
            &record_id,
        );
        set_ccy(&mut rec.margin_currency, attrs);
        return;
    }
    if tail(rel, &["XcssCollPstd"]) || tail(rel, &["XcssCollRcvd"]) {
        set_decimal(&mut rec.excess_collateral, value, "XcssColl", &record_id);
        set_ccy(&mut rec.margin_currency, attrs);
        return;
    }
    // collateral_haircut: auth.108 has no haircut % (only pre/post
    // haircut amounts) → never set. Pre-haircut amounts, CollstnCtgy,
    // Coll/TmStmp and everything else are preserved verbatim.
    if !value.is_empty() {
        rec.raw_fields.insert(rel.join("/"), value.to_owned());
    }
}

fn set_ccy(dst: &mut Option<String>, attrs: &[(String, String)]) {
    if dst.is_none() {
        if let Some(ccy) = attr_of(attrs, "Ccy") {
            *dst = Some(ccy.to_owned());
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

fn set_dt(dst: &mut Option<DateTime<Utc>>, s: &str, field: &str, record: &str) {
    if s.is_empty() {
        return;
    }
    match DateTime::parse_from_rfc3339(s) {
        Ok(d) => *dst = Some(d.with_timezone(&Utc)),
        Err(e) => warn!(record = %record, field, value = s, error = %e, "could not parse datetime"),
    }
}

/// Parse an ISO date (`YYYY-MM-DD`) and store it as midnight UTC.
fn set_date_as_dt(dst: &mut Option<DateTime<Utc>>, s: &str, field: &str, record: &str) {
    if s.is_empty() {
        return;
    }
    match NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|ndt| DateTime::<Utc>::from_naive_utc_and_offset(ndt, Utc))
    {
        Some(dt) => *dst = Some(dt),
        None => warn!(record = %record, field, value = s, "could not parse date"),
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

    const REAL_ENVELOPE: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.108.001.01">
  <DerivsTradMrgnDataRpt>
    <RptHdr><NbRcrds>2</NbRcrds></RptHdr>
    <TradData>
      <Rpt>
        <MrgnUpd>
          <RptgTmStmp>2026-05-13T08:30:00Z</RptgTmStmp>
          <CtrPtyId>
            <RptgCtrPty><Id><Lgl><Id><LEI>RPTGCPARTY0000000001</LEI></Id></Lgl></Id></RptgCtrPty>
            <OthrCtrPty><IdTp><Lgl><Id><LEI>OTHRCPARTY0000000002</LEI></Id></Lgl></IdTp></OthrCtrPty>
          </CtrPtyId>
          <EvtDt>2026-05-12</EvtDt>
          <TxId><UnqTxIdr>MAR-U1</UnqTxIdr></TxId>
          <Coll>
            <CollPrtflCd><Prtfl><Cd>PF-1</Cd></Prtfl></CollPrtflCd>
            <CollstnCtgy>FLCL</CollstnCtgy>
          </Coll>
          <PstdMrgnOrColl>
            <InitlMrgnPstdPstHrcut Ccy="EUR">1000000.00</InitlMrgnPstdPstHrcut>
            <VartnMrgnPstdPstHrcut Ccy="EUR">50000.00</VartnMrgnPstdPstHrcut>
          </PstdMrgnOrColl>
        </MrgnUpd>
      </Rpt>
      <Rpt>
        <Crrctn>
          <RptgTmStmp>2026-05-13T09:00:00Z</RptgTmStmp>
          <TxId><UnqTxIdr>MAR-U2</UnqTxIdr></TxId>
          <Coll><CollPrtflCd><Prtfl><Cd>PF-2</Cd></Prtfl></CollPrtflCd></Coll>
          <RcvdMrgnOrColl>
            <VartnMrgnRcvdPstHrcut Ccy="USD">25000.00</VartnMrgnRcvdPstHrcut>
          </RcvdMrgnOrColl>
        </Crrctn>
      </Rpt>
    </TradData>
  </DerivsTradMrgnDataRpt>
</Document>"#;

    #[test]
    fn parses_real_auth108_envelope() {
        let p = write_tmp("real.xml", REAL_ENVELOPE);
        let out = read_emir_mar_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert!(out.issues.is_empty());
        assert_eq!(out.records.len(), 2);
        let r0 = &out.records[0];
        assert_eq!(r0.uti.as_deref(), Some("MAR-U1"));
        assert_eq!(r0.counterparty_1.as_deref(), Some("RPTGCPARTY0000000001"));
        assert_eq!(r0.counterparty_2.as_deref(), Some("OTHRCPARTY0000000002"));
        assert_eq!(r0.action_type.as_deref(), Some("MRGN"));
        assert_eq!(r0.margin_currency.as_deref(), Some("EUR"));
        assert_eq!(r0.initial_margin_posted.unwrap().to_string(), "1000000.00");
        assert_eq!(r0.variation_margin_posted.unwrap().to_string(), "50000.00");
        assert_eq!(r0.collateral_portfolio_code.as_deref(), Some("PF-1"));
        assert!(r0.reporting_timestamp.is_some());
        // EvtDt (date) normalised to midnight UTC.
        assert_eq!(
            r0.event_timestamp.unwrap().to_rfc3339(),
            "2026-05-12T00:00:00+00:00"
        );
        // No haircut % in auth.108.
        assert!(r0.collateral_haircut.is_none());

        let r1 = &out.records[1];
        assert_eq!(r1.action_type.as_deref(), Some("CORR"));
        assert_eq!(
            r1.variation_margin_collected.unwrap().to_string(),
            "25000.00"
        );
        assert_eq!(r1.margin_currency.as_deref(), Some("USD"));
    }

    #[test]
    fn empty_report_no_records_info() {
        let body = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.108.001.01">
  <DerivsTradMrgnDataRpt>
    <RptHdr><NbRcrds>0</NbRcrds></RptHdr>
    <TradData><DataSetActn>NOTX</DataSetActn></TradData>
  </DerivsTradMrgnDataRpt>
</Document>"#;
        let p = write_tmp("empty.xml", body);
        let out = read_emir_mar_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert!(out.records.is_empty());
        assert_eq!(out.issues.len(), 1);
        assert_eq!(out.issues[0].check_id, "EMIR.FMT.MAR_NO_RECORDS");
        assert_eq!(out.issues[0].severity, Severity::Info);
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
