//! SFTR Margin Data Transaction Report (MAR) ingestion —
//! ISO 20022 `auth.070`.
//!
//! Element paths are aligned with the real ESMA SFTR usage guideline
//! `auth.070.001.02_ESMAUG_1.1.0`
//! (`SecuritiesFinancingReportingTransactionMarginDataReportV02`).
//! The SWIFT-licensed XSD is **not** redistributed; only the schema
//! *shape* (element names, nesting, cardinalities) is encoded here.
//! Coverage is intentionally a documented subset — see
//! `docs/auth-messages/sftr-auth070.md` (added in Phase H).
//!
//! Real envelope:
//! ```text
//! Document
//! └─ SctiesFincgRptgTxMrgnDataRpt        (SecuritiesFinancingReportingTransactionMarginDataReportV02)
//!    └─ TradData                          (choice)
//!       ├─ DataSetActn = "NOTX"           (empty / no-activity report)
//!       └─ Rpt  (1..n)                    (TradeReport21Choice__1 — action-typed wrapper)
//!          └─ <choice: New | Err | Crrctn | TradUpd>
//!             ├─ TechRcrdId                                (Max140Text)
//!             ├─ RptgDtTm                                  (ISONormalisedDateTime, per-event "state as of")
//!             ├─ EvtDt                                     (ISODate, absent in Err)
//!             ├─ CtrPty (Counterparty39__1 or __2)
//!             │  ├─ RptgCtrPty/<choice>/LEI                → reporting_counterparty
//!             │  └─ OthrCtrPty/<choice>/{Lgl/LEI | Ntrl/Id/Id}  → other_counterparty
//!             ├─ CollPrtflId                               (Max52Text, mandatory)
//!             ├─ PstdMrgnOrColl  [0..1] (PostedMarginOrCollateral4 — absent in Err)
//!             │  ├─ InitlMrgnPstd/Amt(@Ccy)
//!             │  ├─ VartnMrgnPstd/Amt(@Ccy)
//!             │  └─ XcssCollPstd/Amt(@Ccy)
//!             └─ RcvdMrgnOrColl  [0..1] (ReceivedMarginOrCollateral4 — absent in Err)
//!                ├─ InitlMrgnRcvd/Amt(@Ccy)
//!                ├─ VartnMrgnRcvd/Amt(@Ccy)
//!                └─ XcssCollRcvd/Amt(@Ccy)
//! ```
//!
//! Unlike auth.085 (portfolio-level state snapshot, `CtrctMod/ActnTp`
//! leaf), auth.070 is an **event-driven activity report**: the action
//! type is encoded in the wrapper element name itself —
//! `New` → `NEWT`, `Err` → `ERRT`, `Crrctn` → `CORR`, `TradUpd` →
//! `TRDU`. The shape inside the wrapper is otherwise identical to
//! the per-record `Stat` block of auth.085 (same counterparty choice,
//! same `CollPrtflId`, same 6 amounts). The `Err` wrapper carries
//! only metadata (no amounts, no `EvtDt`).
//!
//! Sister parser: `sftr_margin_state.rs` (auth.085 — state snapshot).

use std::path::Path;
use std::str::FromStr;

use chrono::{DateTime, NaiveDate, Utc};
use opendqi_core::{DqDimension, DqIssue, Regime, Severity, SftrMarginActivityRecord};
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::NsReader;
use rust_decimal::Decimal;
use tracing::warn;

use crate::wellformed::check_wellformedness;

/// Outcome of reading one SFTR MAR XML file.
#[derive(Debug, Default)]
pub struct SftrMarginActivityXmlReadOutcome {
    /// Records extracted from the file.
    pub records: Vec<SftrMarginActivityRecord>,
    /// File-level data-quality / parse issues (format / namespace).
    pub issues: Vec<DqIssue>,
}

const ISO20022_AUTH_070_NS: &[u8] = b"urn:iso:std:iso:20022:tech:xsd:auth.070.001.02";
/// The repeating per-record element under `TradData` in auth.070.
const RPT_BLOCK: &str = "Rpt";

/// Read an SFTR `auth.070` Margin Data Transaction Report file.
pub fn read_sftr_margin_activity_xml(
    path: &Path,
) -> anyhow::Result<SftrMarginActivityXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();

    if let Err(err) = check_wellformedness(path) {
        return Ok(SftrMarginActivityXmlReadOutcome {
            records: vec![],
            issues: vec![fmt_issue(
                "SFTR.FMT.XML_NOT_WELLFORMED",
                Severity::Critical,
                format!("XML is not well-formed: {}", err.message),
                source_label,
            )],
        });
    }

    match peek_root_namespace(path)? {
        Some(ns) if ns == ISO20022_AUTH_070_NS => parse(path),
        other => {
            let actual = other
                .as_deref()
                .map(|n| String::from_utf8_lossy(n).into_owned())
                .unwrap_or_else(|| "(none)".into());
            Ok(SftrMarginActivityXmlReadOutcome {
                records: vec![],
                issues: vec![fmt_issue(
                    "SFTR.FMT.XML_UNSUPPORTED_NAMESPACE",
                    Severity::Warning,
                    format!(
                        "Root namespace is '{actual}', expected 'urn:iso:std:iso:20022:tech:xsd:auth.070.001.02'."
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
        regime: Regime::Sftr,
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

/// Map the wrapper element name to the canonical 4-letter action_type
/// code. Returns `None` when the element is not one of the 4 known
/// `TradeReport21Choice__1` choice branches.
fn wrapper_action_type(local: &str) -> Option<&'static str> {
    match local {
        "New" => Some("NEWT"),
        "Err" => Some("ERRT"),
        "Crrctn" => Some("CORR"),
        "TradUpd" => Some("TRDU"),
        _ => None,
    }
}

fn parse(path: &Path) -> anyhow::Result<SftrMarginActivityXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();
    let mut reader = NsReader::from_file(path)?;
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut pile: Vec<String> = Vec::new();
    let mut is_leaf: Vec<bool> = Vec::new();
    let mut text_buf = String::new();
    let mut attrs_buf: Vec<(String, String)> = Vec::new();

    let mut current: Option<SftrMarginActivityRecord> = None;
    // Depth at which the *wrapper* element (New/Err/Crrctn/TradUpd)
    // sits. Leaves inside the record are at `pile[wrapper_depth..]`.
    let mut rec_depth: Option<usize> = None;
    let mut records: Vec<SftrMarginActivityRecord> = Vec::new();
    let mut rec_index: u32 = 0;
    let mut saw_dataset_actn = false;

    loop {
        match reader.read_resolved_event_into(&mut buf)? {
            (_, Event::Start(e)) => {
                let local = local_name(&e);
                push_element(&mut pile, &mut is_leaf, local.clone());
                text_buf.clear();
                attrs_buf = collect_attrs(&e);

                // Record begins at the *wrapper* depth, which is one
                // level under `Rpt`. We detect that pattern and stamp
                // `action_type` from the wrapper name on the spot.
                if current.is_none()
                    && pile.len() >= 2
                    && pile[pile.len() - 2] == RPT_BLOCK
                    && pile.iter().any(|s| s == "TradData")
                {
                    if let Some(actn) = wrapper_action_type(&local) {
                        rec_index += 1;
                        current = Some(SftrMarginActivityRecord {
                            source_file: Some(source_label.clone()),
                            record_id: Some(format!("{source_label}#rpt-{rec_index}")),
                            regime: Regime::Sftr,
                            action_type: Some(actn.to_owned()),
                            ..Default::default()
                        });
                        rec_depth = Some(pile.len());
                    }
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
            "SFTR.FMT.SFTR_MAR_NO_RECORDS",
            Severity::Info,
            "SFTR Margin Data Transaction Report carries \
             TradData/DataSetActn (no-activity report); zero margin \
             activity records to evaluate."
                .to_string(),
            source_label,
        ));
    }

    Ok(SftrMarginActivityXmlReadOutcome { records, issues })
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

/// Promote the per-amount `@Ccy` attribute onto `margin_currency`
/// the first time we see it. Mirrors the auth.085 first-wins
/// strategy; divergence is signalled by an `SFTR.MAR.*` granular
/// check (added in A5).
fn capture_ccy(rec: &mut SftrMarginActivityRecord, attrs: &[(String, String)]) {
    if rec.margin_currency.is_none() {
        if let Some(ccy) = attr_of(attrs, "Ccy") {
            rec.margin_currency = Some(ccy.to_owned());
        }
    }
}

/// Map one leaf (path relative to the wrapper element — i.e. relative
/// to `New`/`Err`/`Crrctn`/`TradUpd`) onto the canonical
/// `SftrMarginActivityRecord`. Real `auth.070.001.02` element paths;
/// every other branch is intentionally not extracted (documented in
/// `docs/auth-messages/sftr-auth070.md`, Phase H).
fn commit_leaf(
    rec: &mut SftrMarginActivityRecord,
    rel: &[String],
    value: &str,
    attrs: &[(String, String)],
) {
    if value.is_empty() && attrs.is_empty() {
        return;
    }
    let record_id = rec.record_id.clone().unwrap_or_default();

    // Header fields (not gated by any ancestor in the per-wrapper scope).
    if rel.last().map(String::as_str) == Some("TechRcrdId") {
        if !value.is_empty() {
            rec.record_id = Some(value.to_owned());
        }
        return;
    }
    if rel.last().map(String::as_str) == Some("RptgDtTm") {
        let mut tmp = None;
        set_dt(&mut tmp, value, "RptgDtTm", &record_id);
        if tmp.is_some() {
            rec.state_as_of = tmp;
        }
        return;
    }
    if rel.last().map(String::as_str) == Some("EvtDt") {
        // Absent in the `Err` wrapper — kept Option<NaiveDate>.
        set_date(&mut rec.event_date, value, "EvtDt", &record_id);
        return;
    }
    if rel.last().map(String::as_str) == Some("CollPrtflId") {
        if !value.is_empty() {
            rec.collateral_portfolio_code = Some(value.to_owned());
        }
        return;
    }

    // Counterparties: LEI tail anywhere under RptgCtrPty / OthrCtrPty.
    // Same shape as auth.085 inside the wrapper.
    if tail(rel, &["LEI"]) && has(rel, "RptgCtrPty") {
        rec.reporting_counterparty = Some(value.to_owned());
        return;
    }
    if tail(rel, &["LEI"]) && has(rel, "OthrCtrPty") {
        rec.other_counterparty = Some(value.to_owned());
        return;
    }
    if tail(rel, &["Ntrl", "Id"]) && has(rel, "OthrCtrPty") {
        if rec.other_counterparty.is_none() {
            rec.other_counterparty = Some(value.to_owned());
        }
        return;
    }

    // T3 margin amounts (6 total) under PstdMrgnOrColl / RcvdMrgnOrColl
    // — same shape and same field mapping as auth.085. Absent in the
    // `Err` wrapper at the XSD level (the parser tolerates it: those
    // tails simply never fire).
    if tail(rel, &["InitlMrgnPstd", "Amt"]) && has(rel, "PstdMrgnOrColl") {
        set_decimal(
            &mut rec.initial_margin_posted,
            value,
            "initial_margin_posted",
            &record_id,
        );
        capture_ccy(rec, attrs);
        return;
    }
    if tail(rel, &["VartnMrgnPstd", "Amt"]) && has(rel, "PstdMrgnOrColl") {
        set_decimal(
            &mut rec.variation_margin_posted,
            value,
            "variation_margin_posted",
            &record_id,
        );
        capture_ccy(rec, attrs);
        return;
    }
    if tail(rel, &["XcssCollPstd", "Amt"]) && has(rel, "PstdMrgnOrColl") {
        set_decimal(
            &mut rec.excess_collateral_posted,
            value,
            "excess_collateral_posted",
            &record_id,
        );
        capture_ccy(rec, attrs);
        return;
    }
    if tail(rel, &["InitlMrgnRcvd", "Amt"]) && has(rel, "RcvdMrgnOrColl") {
        set_decimal(
            &mut rec.initial_margin_received,
            value,
            "initial_margin_received",
            &record_id,
        );
        capture_ccy(rec, attrs);
        return;
    }
    if tail(rel, &["VartnMrgnRcvd", "Amt"]) && has(rel, "RcvdMrgnOrColl") {
        set_decimal(
            &mut rec.variation_margin_received,
            value,
            "variation_margin_received",
            &record_id,
        );
        capture_ccy(rec, attrs);
        return;
    }
    if tail(rel, &["XcssCollRcvd", "Amt"]) && has(rel, "RcvdMrgnOrColl") {
        set_decimal(
            &mut rec.excess_collateral_received,
            value,
            "excess_collateral_received",
            &record_id,
        );
        capture_ccy(rec, attrs);
        return;
    }

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
            std::env::temp_dir().join(format!("opendqi-sftr-mar-{}-{name}", std::process::id()));
        std::fs::write(&p, content).unwrap();
        p
    }

    const NEW_FULL_RECORD: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.070.001.02">
  <SctiesFincgRptgTxMrgnDataRpt>
    <TradData>
      <Rpt>
        <New>
          <TechRcrdId>REC-NEW-1</TechRcrdId>
          <RptgDtTm>2026-05-13T08:00:00Z</RptgDtTm>
          <EvtDt>2026-05-12</EvtDt>
          <CtrPty>
            <RptgCtrPty><Id><LEI>RPTGCPARTY0000000001</LEI></Id></RptgCtrPty>
            <OthrCtrPty><Id><Lgl><LEI>OTHRCPARTY0000000002</LEI></Lgl></Id></OthrCtrPty>
          </CtrPty>
          <CollPrtflId>PORTFOLIO-001</CollPrtflId>
          <PstdMrgnOrColl>
            <InitlMrgnPstd><Amt Ccy="EUR">1000.50</Amt></InitlMrgnPstd>
            <VartnMrgnPstd><Amt Ccy="EUR">50.00</Amt></VartnMrgnPstd>
            <XcssCollPstd><Amt Ccy="EUR">25.25</Amt></XcssCollPstd>
          </PstdMrgnOrColl>
          <RcvdMrgnOrColl>
            <InitlMrgnRcvd><Amt Ccy="EUR">980.75</Amt></InitlMrgnRcvd>
            <VartnMrgnRcvd><Amt Ccy="EUR">48.00</Amt></VartnMrgnRcvd>
            <XcssCollRcvd><Amt Ccy="EUR">20.10</Amt></XcssCollRcvd>
          </RcvdMrgnOrColl>
        </New>
      </Rpt>
    </TradData>
  </SctiesFincgRptgTxMrgnDataRpt>
</Document>"#;

    #[test]
    fn parses_new_wrapper_full_record_all_6_amounts_action_newt() {
        let p = write_tmp("new_full.xml", NEW_FULL_RECORD);
        let outcome = read_sftr_margin_activity_xml(&p).expect("parse");
        assert!(
            outcome.issues.is_empty(),
            "no format issues expected: {:?}",
            outcome.issues
        );
        assert_eq!(outcome.records.len(), 1);
        let r = &outcome.records[0];
        assert_eq!(r.record_id.as_deref(), Some("REC-NEW-1"));
        assert_eq!(r.regime, Regime::Sftr);
        assert_eq!(r.action_type.as_deref(), Some("NEWT"));
        assert_eq!(
            r.reporting_counterparty.as_deref(),
            Some("RPTGCPARTY0000000001")
        );
        assert_eq!(
            r.other_counterparty.as_deref(),
            Some("OTHRCPARTY0000000002")
        );
        assert_eq!(
            r.collateral_portfolio_code.as_deref(),
            Some("PORTFOLIO-001")
        );
        assert_eq!(r.margin_currency.as_deref(), Some("EUR"));
        assert_eq!(
            r.initial_margin_posted,
            Some(Decimal::from_str("1000.50").unwrap())
        );
        assert_eq!(
            r.variation_margin_posted,
            Some(Decimal::from_str("50.00").unwrap())
        );
        assert_eq!(
            r.excess_collateral_posted,
            Some(Decimal::from_str("25.25").unwrap())
        );
        assert_eq!(
            r.initial_margin_received,
            Some(Decimal::from_str("980.75").unwrap())
        );
        assert_eq!(
            r.variation_margin_received,
            Some(Decimal::from_str("48.00").unwrap())
        );
        assert_eq!(
            r.excess_collateral_received,
            Some(Decimal::from_str("20.10").unwrap())
        );
        assert!(r.event_date.is_some());
        assert!(r.state_as_of.is_some());
        let _ = std::fs::remove_file(&p);
    }

    const ALL_FOUR_WRAPPERS: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.070.001.02">
  <SctiesFincgRptgTxMrgnDataRpt>
    <TradData>
      <Rpt><New>
        <TechRcrdId>R-NEW</TechRcrdId>
        <RptgDtTm>2026-05-13T08:00:00Z</RptgDtTm>
        <EvtDt>2026-05-12</EvtDt>
        <CtrPty>
          <RptgCtrPty><Id><LEI>LEI-A</LEI></Id></RptgCtrPty>
          <OthrCtrPty><Id><Lgl><LEI>LEI-B</LEI></Lgl></Id></OthrCtrPty>
        </CtrPty>
        <CollPrtflId>PRT-1</CollPrtflId>
      </New></Rpt>
      <Rpt><Crrctn>
        <TechRcrdId>R-CORR</TechRcrdId>
        <RptgDtTm>2026-05-13T08:00:00Z</RptgDtTm>
        <EvtDt>2026-05-12</EvtDt>
        <CtrPty>
          <RptgCtrPty><Id><LEI>LEI-A</LEI></Id></RptgCtrPty>
          <OthrCtrPty><Id><Lgl><LEI>LEI-B</LEI></Lgl></Id></OthrCtrPty>
        </CtrPty>
        <CollPrtflId>PRT-1</CollPrtflId>
      </Crrctn></Rpt>
      <Rpt><TradUpd>
        <TechRcrdId>R-TRD</TechRcrdId>
        <RptgDtTm>2026-05-13T08:00:00Z</RptgDtTm>
        <EvtDt>2026-05-12</EvtDt>
        <CtrPty>
          <RptgCtrPty><Id><LEI>LEI-A</LEI></Id></RptgCtrPty>
          <OthrCtrPty><Id><Lgl><LEI>LEI-B</LEI></Lgl></Id></OthrCtrPty>
        </CtrPty>
        <CollPrtflId>PRT-1</CollPrtflId>
      </TradUpd></Rpt>
      <Rpt><Err>
        <TechRcrdId>R-ERR</TechRcrdId>
        <RptgDtTm>2026-05-13T08:00:00Z</RptgDtTm>
        <CtrPty>
          <RptgCtrPty><Id><LEI>LEI-A</LEI></Id></RptgCtrPty>
          <OthrCtrPty><Id><Lgl><LEI>LEI-B</LEI></Lgl></Id></OthrCtrPty>
        </CtrPty>
        <CollPrtflId>PRT-1</CollPrtflId>
      </Err></Rpt>
    </TradData>
  </SctiesFincgRptgTxMrgnDataRpt>
</Document>"#;

    #[test]
    fn four_wrappers_map_to_canonical_action_codes_err_has_no_evtdt() {
        let p = write_tmp("four_wrappers.xml", ALL_FOUR_WRAPPERS);
        let outcome = read_sftr_margin_activity_xml(&p).expect("parse");
        assert!(outcome.issues.is_empty(), "no format issues");
        assert_eq!(outcome.records.len(), 4);
        let actions: Vec<&str> = outcome
            .records
            .iter()
            .map(|r| r.action_type.as_deref().unwrap())
            .collect();
        assert_eq!(actions, vec!["NEWT", "CORR", "TRDU", "ERRT"]);
        // `Err` wrapper has no EvtDt per XSD; others do.
        let err = outcome
            .records
            .iter()
            .find(|r| r.action_type.as_deref() == Some("ERRT"))
            .unwrap();
        assert!(err.event_date.is_none(), "Err wrapper has no EvtDt");
        for r in outcome
            .records
            .iter()
            .filter(|r| r.action_type.as_deref() != Some("ERRT"))
        {
            assert!(r.event_date.is_some(), "non-Err wrappers have EvtDt");
        }
        let _ = std::fs::remove_file(&p);
    }

    const POSTED_ONLY_NEW: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.070.001.02">
  <SctiesFincgRptgTxMrgnDataRpt>
    <TradData>
      <Rpt><New>
        <TechRcrdId>R-POSTED</TechRcrdId>
        <RptgDtTm>2026-05-13T08:00:00Z</RptgDtTm>
        <EvtDt>2026-05-12</EvtDt>
        <CtrPty>
          <RptgCtrPty><Id><LEI>LEI-A</LEI></Id></RptgCtrPty>
          <OthrCtrPty><Id><Lgl><LEI>LEI-B</LEI></Lgl></Id></OthrCtrPty>
        </CtrPty>
        <CollPrtflId>PRT-POSTED</CollPrtflId>
        <PstdMrgnOrColl>
          <InitlMrgnPstd><Amt Ccy="USD">500.00</Amt></InitlMrgnPstd>
          <VartnMrgnPstd><Amt Ccy="USD">10.00</Amt></VartnMrgnPstd>
        </PstdMrgnOrColl>
      </New></Rpt>
    </TradData>
  </SctiesFincgRptgTxMrgnDataRpt>
</Document>"#;

    #[test]
    fn posted_only_record_received_amounts_stay_none() {
        let p = write_tmp("posted_only.xml", POSTED_ONLY_NEW);
        let outcome = read_sftr_margin_activity_xml(&p).expect("parse");
        assert_eq!(outcome.records.len(), 1);
        let r = &outcome.records[0];
        assert_eq!(r.action_type.as_deref(), Some("NEWT"));
        assert_eq!(r.margin_currency.as_deref(), Some("USD"));
        assert!(r.initial_margin_posted.is_some());
        assert!(r.variation_margin_posted.is_some());
        assert!(r.excess_collateral_posted.is_none());
        assert!(r.initial_margin_received.is_none());
        assert!(r.variation_margin_received.is_none());
        assert!(r.excess_collateral_received.is_none());
        let _ = std::fs::remove_file(&p);
    }

    const NOTX: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.070.001.02">
  <SctiesFincgRptgTxMrgnDataRpt>
    <TradData>
      <DataSetActn>NOTX</DataSetActn>
    </TradData>
  </SctiesFincgRptgTxMrgnDataRpt>
</Document>"#;

    #[test]
    fn no_records_with_dataset_actn_emits_info_issue() {
        let p = write_tmp("notx.xml", NOTX);
        let outcome = read_sftr_margin_activity_xml(&p).expect("parse");
        assert!(outcome.records.is_empty());
        assert_eq!(outcome.issues.len(), 1);
        assert_eq!(outcome.issues[0].check_id, "SFTR.FMT.SFTR_MAR_NO_RECORDS");
        assert_eq!(outcome.issues[0].severity, Severity::Info);
        let _ = std::fs::remove_file(&p);
    }

    const WRONG_NS: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.999.001.02">
  <SomethingElse/>
</Document>"#;

    #[test]
    fn wrong_namespace_emits_warning_no_records() {
        let p = write_tmp("wrong_ns.xml", WRONG_NS);
        let outcome = read_sftr_margin_activity_xml(&p).expect("parse");
        assert!(outcome.records.is_empty());
        assert_eq!(outcome.issues.len(), 1);
        assert_eq!(
            outcome.issues[0].check_id,
            "SFTR.FMT.XML_UNSUPPORTED_NAMESPACE"
        );
        let _ = std::fs::remove_file(&p);
    }

    const NATURAL_PERSON: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.070.001.02">
  <SctiesFincgRptgTxMrgnDataRpt>
    <TradData>
      <Rpt><New>
        <TechRcrdId>R-NTRL</TechRcrdId>
        <RptgDtTm>2026-05-13T08:00:00Z</RptgDtTm>
        <EvtDt>2026-05-12</EvtDt>
        <CtrPty>
          <RptgCtrPty><Id><LEI>LEI-A</LEI></Id></RptgCtrPty>
          <OthrCtrPty><Id><Ntrl><Id>NATURAL-PERSON-ID-42</Id></Ntrl></Id></OthrCtrPty>
        </CtrPty>
        <CollPrtflId>PRT-NTRL</CollPrtflId>
      </New></Rpt>
    </TradData>
  </SctiesFincgRptgTxMrgnDataRpt>
</Document>"#;

    #[test]
    fn other_counterparty_natural_person_id_falls_through_lei_path() {
        let p = write_tmp("ntrl.xml", NATURAL_PERSON);
        let outcome = read_sftr_margin_activity_xml(&p).expect("parse");
        assert_eq!(outcome.records.len(), 1);
        let r = &outcome.records[0];
        assert_eq!(
            r.other_counterparty.as_deref(),
            Some("NATURAL-PERSON-ID-42")
        );
        let _ = std::fs::remove_file(&p);
    }
}
