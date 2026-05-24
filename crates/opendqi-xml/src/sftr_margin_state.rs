//! SFTR Margin Data Transaction State Report (MSR) ingestion —
//! ISO 20022 `auth.085`.
//!
//! Element paths are aligned with the real ESMA SFTR usage guideline
//! `auth.085.001.02_ESMAUG_1.0.0`
//! (`SecuritiesFinancingReportingMarginDataTransactionStateReportV02`).
//! The SWIFT-licensed XSD is **not** redistributed; only the schema
//! *shape* (element names, nesting, cardinalities) is encoded here.
//! Coverage is intentionally a documented subset — see
//! `docs/auth-messages/sftr-auth085.md`.
//!
//! Real envelope:
//! ```text
//! Document
//! └─ SctiesFincgRptgMrgnDataTxStatRpt    (SecuritiesFinancingReportingMarginDataTransactionStateReportV02)
//!    └─ TradData  (choice)
//!       ├─ DataSetActn = "NOTX"  (empty / no-activity report)
//!       └─ Stat  (1..n)          (CollateralMarginNew10__1)
//!          ├─ TechRcrdId                                (Max140Text)
//!          ├─ RptgDtTm                                  (ISODateTime, per-record "state as of")
//!          ├─ EvtDt                                     (ISODate)
//!          ├─ CtrPty (Counterparty39__1)
//!          │  ├─ RptgCtrPty/<choice>/LEI                → reporting_counterparty
//!          │  └─ OthrCtrPty/<choice>/{Lgl/LEI | Ntrl/Id/Id}  → other_counterparty
//!          ├─ CollPrtflId                               (Max52Text, mandatory)
//!          ├─ PstdMrgnOrColl  [0..1] (PostedMarginOrCollateral4)
//!          │  ├─ InitlMrgnPstd/Amt(@Ccy)
//!          │  ├─ VartnMrgnPstd/Amt(@Ccy)
//!          │  └─ XcssCollPstd/Amt(@Ccy)
//!          ├─ RcvdMrgnOrColl  [0..1] (ReceivedMarginOrCollateral4)
//!          │  ├─ InitlMrgnRcvd/Amt(@Ccy)
//!          │  ├─ VartnMrgnRcvd/Amt(@Ccy)
//!          │  └─ XcssCollRcvd/Amt(@Ccy)
//!          └─ CtrctMod/ActnTp                           → action_type
//! ```
//!
//! Note: `auth.085` is portfolio-level (indexed by `CollPrtflId`,
//! **not** by UTI — `CollateralMarginNew10__1` has no `UnqTradIdr`
//! element) and is restricted to CCP-cleared SFTs per the ESMA
//! documentation. SFTR has 6 amounts, no pre/post-haircut split
//! (unlike EMIR auth.109 which has 4 amounts with pre-haircut
//! distinct from post-haircut). The 6 amounts share a single
//! `margin_currency` promoted from the per-amount `@Ccy` attribute
//! (first-wins: if amounts disagree the first one observed sets
//! the field, divergence is signalled by a granular check). The
//! single mapping point is `commit_leaf`.

use std::path::Path;
use std::str::FromStr;

use chrono::{DateTime, NaiveDate, Utc};
use opendqi_core::{DqDimension, DqIssue, Regime, Severity, SftrMarginStateRecord};
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::NsReader;
use rust_decimal::Decimal;
use tracing::warn;

use crate::wellformed::check_wellformedness;

/// Outcome of reading one SFTR MSR XML file.
#[derive(Debug, Default)]
pub struct SftrMarginStateXmlReadOutcome {
    /// Records extracted from the file.
    pub records: Vec<SftrMarginStateRecord>,
    /// File-level data-quality / parse issues (format / namespace).
    pub issues: Vec<DqIssue>,
}

const ISO20022_AUTH_085_NS: &[u8] = b"urn:iso:std:iso:20022:tech:xsd:auth.085.001.02";
/// The repeating per-record element under `TradData` in auth.085.
const STAT_BLOCK: &str = "Stat";

/// Read an SFTR `auth.085` Margin Data Transaction State Report file.
pub fn read_sftr_margin_state_xml(path: &Path) -> anyhow::Result<SftrMarginStateXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();

    if let Err(err) = check_wellformedness(path) {
        return Ok(SftrMarginStateXmlReadOutcome {
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
        Some(ns) if ns == ISO20022_AUTH_085_NS => parse(path),
        other => {
            let actual = other
                .as_deref()
                .map(|n| String::from_utf8_lossy(n).into_owned())
                .unwrap_or_else(|| "(none)".into());
            Ok(SftrMarginStateXmlReadOutcome {
                records: vec![],
                issues: vec![fmt_issue(
                    "SFTR.FMT.XML_UNSUPPORTED_NAMESPACE",
                    Severity::Warning,
                    format!(
                        "Root namespace is '{actual}', expected 'urn:iso:std:iso:20022:tech:xsd:auth.085.001.02'."
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

fn parse(path: &Path) -> anyhow::Result<SftrMarginStateXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();
    let mut reader = NsReader::from_file(path)?;
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut pile: Vec<String> = Vec::new();
    let mut is_leaf: Vec<bool> = Vec::new();
    let mut text_buf = String::new();
    let mut attrs_buf: Vec<(String, String)> = Vec::new();

    let mut current: Option<SftrMarginStateRecord> = None;
    let mut rec_depth: Option<usize> = None;
    let mut records: Vec<SftrMarginStateRecord> = Vec::new();
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
                    current = Some(SftrMarginStateRecord {
                        source_file: Some(source_label.clone()),
                        record_id: Some(format!("{source_label}#stat-{rec_index}")),
                        regime: Regime::Sftr,
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
            "SFTR.FMT.SFTR_MSR_NO_RECORDS",
            Severity::Info,
            "SFTR Margin Data Transaction State Report carries \
             TradData/DataSetActn (no-activity report); zero margin \
             state records to evaluate."
                .to_string(),
            source_label,
        ));
    }

    Ok(SftrMarginStateXmlReadOutcome { records, issues })
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
/// the first time we see it. The XSD allows divergence across the
/// 6 amounts but in practice a portfolio has a single margining
/// currency; a `SFTR.T3.MARGIN_CURRENCY_*` granular check (added
/// in F1') flags any divergence.
fn capture_ccy(rec: &mut SftrMarginStateRecord, attrs: &[(String, String)]) {
    if rec.margin_currency.is_none() {
        if let Some(ccy) = attr_of(attrs, "Ccy") {
            rec.margin_currency = Some(ccy.to_owned());
        }
    }
}

/// Map one leaf (path relative to the `Stat` record) onto the canonical
/// `SftrMarginStateRecord`. Real `auth.085.001.02` element paths; every
/// other branch is intentionally not extracted (documented in
/// `docs/auth-messages/sftr-auth085.md`).
fn commit_leaf(
    rec: &mut SftrMarginStateRecord,
    rel: &[String],
    value: &str,
    attrs: &[(String, String)],
) {
    if value.is_empty() && attrs.is_empty() {
        return;
    }
    let record_id = rec.record_id.clone().unwrap_or_default();

    // Header fields (not gated by any ancestor in the per-Stat scope).
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
        set_date(&mut rec.event_date, value, "EvtDt", &record_id);
        return;
    }
    if rel.last().map(String::as_str) == Some("CollPrtflId") {
        if !value.is_empty() {
            rec.collateral_portfolio_code = Some(value.to_owned());
        }
        return;
    }
    if rel.last().map(String::as_str) == Some("ActnTp") && has(rel, "CtrctMod") {
        if !value.is_empty() {
            rec.action_type = Some(value.to_owned());
        }
        return;
    }

    // Counterparties: LEI tail anywhere under RptgCtrPty / OthrCtrPty.
    // OthrCtrPty has a choice with Ntrl/Id/Id (natural person) — also
    // captured as the same string field.
    if tail(rel, &["LEI"]) && has(rel, "RptgCtrPty") {
        rec.reporting_counterparty = Some(value.to_owned());
        return;
    }
    if tail(rel, &["LEI"]) && has(rel, "OthrCtrPty") {
        rec.other_counterparty = Some(value.to_owned());
        return;
    }
    if tail(rel, &["Ntrl", "Id"]) && has(rel, "OthrCtrPty") {
        // OthrCtrPty/Id/Ntrl/Id — the inner Id wraps the natural-person
        // identifier text value. The leaf element name is `Id` and its
        // immediate parent is `Ntrl`, so the 2-element tail is the
        // unique signature.
        if rec.other_counterparty.is_none() {
            rec.other_counterparty = Some(value.to_owned());
        }
        return;
    }

    // T3 margin amounts (6 total) under PstdMrgnOrColl / RcvdMrgnOrColl.
    // Each carries an @Ccy attribute on the Amt element.
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
            std::env::temp_dir().join(format!("opendqi-sftr-msr-{}-{name}", std::process::id()));
        std::fs::write(&p, content).unwrap();
        p
    }

    const FULL_RECORD: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.085.001.02">
  <SctiesFincgRptgMrgnDataTxStatRpt>
    <TradData>
      <Stat>
        <TechRcrdId>REC-1</TechRcrdId>
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
        <CtrctMod><ActnTp>MARU</ActnTp></CtrctMod>
      </Stat>
    </TradData>
  </SctiesFincgRptgMrgnDataTxStatRpt>
</Document>"#;

    #[test]
    fn parses_full_record_all_6_amounts_plus_currency_and_metadata() {
        let p = write_tmp("full.xml", FULL_RECORD);
        let outcome = read_sftr_margin_state_xml(&p).expect("parse");
        assert!(
            outcome.issues.is_empty(),
            "no format issues expected: {:?}",
            outcome.issues
        );
        assert_eq!(outcome.records.len(), 1);
        let r = &outcome.records[0];
        // TechRcrdId overrides the synthetic stat-N id.
        assert_eq!(r.record_id.as_deref(), Some("REC-1"));
        assert_eq!(r.regime, Regime::Sftr);
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
        assert_eq!(r.action_type.as_deref(), Some("MARU"));
        // 6 amounts + shared @Ccy.
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

    const POSTED_ONLY_RECORD: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.085.001.02">
  <SctiesFincgRptgMrgnDataTxStatRpt>
    <TradData>
      <Stat>
        <TechRcrdId>REC-PARTIAL</TechRcrdId>
        <RptgDtTm>2026-05-13T08:00:00Z</RptgDtTm>
        <EvtDt>2026-05-12</EvtDt>
        <CtrPty>
          <RptgCtrPty><Id><LEI>RPTGCPARTY0000000001</LEI></Id></RptgCtrPty>
          <OthrCtrPty><Id><Lgl><LEI>OTHRCPARTY0000000002</LEI></Lgl></Id></OthrCtrPty>
        </CtrPty>
        <CollPrtflId>PORTFOLIO-002</CollPrtflId>
        <PstdMrgnOrColl>
          <InitlMrgnPstd><Amt Ccy="USD">500.00</Amt></InitlMrgnPstd>
          <VartnMrgnPstd><Amt Ccy="USD">10.00</Amt></VartnMrgnPstd>
        </PstdMrgnOrColl>
        <CtrctMod><ActnTp>NEWT</ActnTp></CtrctMod>
      </Stat>
    </TradData>
  </SctiesFincgRptgMrgnDataTxStatRpt>
</Document>"#;

    #[test]
    fn parses_posted_only_record_received_amounts_stay_none() {
        // Real-world: a portfolio may report posted but not received
        // (or vice versa) — both blocks are [0..1] in the XSD. The
        // parser must leave the absent amounts at None, not zero.
        let p = write_tmp("posted_only.xml", POSTED_ONLY_RECORD);
        let outcome = read_sftr_margin_state_xml(&p).expect("parse");
        assert_eq!(outcome.records.len(), 1);
        let r = &outcome.records[0];
        assert_eq!(r.margin_currency.as_deref(), Some("USD"));
        assert!(r.initial_margin_posted.is_some());
        assert!(r.variation_margin_posted.is_some());
        assert!(r.excess_collateral_posted.is_none()); // not in fixture
        assert!(r.initial_margin_received.is_none());
        assert!(r.variation_margin_received.is_none());
        assert!(r.excess_collateral_received.is_none());
        let _ = std::fs::remove_file(&p);
    }

    const NOTX_RECORD: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.085.001.02">
  <SctiesFincgRptgMrgnDataTxStatRpt>
    <TradData>
      <DataSetActn>NOTX</DataSetActn>
    </TradData>
  </SctiesFincgRptgMrgnDataTxStatRpt>
</Document>"#;

    #[test]
    fn no_records_with_dataset_actn_emits_info_issue() {
        let p = write_tmp("notx.xml", NOTX_RECORD);
        let outcome = read_sftr_margin_state_xml(&p).expect("parse");
        assert!(outcome.records.is_empty());
        assert_eq!(outcome.issues.len(), 1);
        assert_eq!(outcome.issues[0].check_id, "SFTR.FMT.SFTR_MSR_NO_RECORDS");
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
        let outcome = read_sftr_margin_state_xml(&p).expect("parse");
        assert!(outcome.records.is_empty());
        assert_eq!(outcome.issues.len(), 1);
        assert_eq!(
            outcome.issues[0].check_id,
            "SFTR.FMT.XML_UNSUPPORTED_NAMESPACE"
        );
        let _ = std::fs::remove_file(&p);
    }

    const NATURAL_PERSON_OTHRCTRPTY: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.085.001.02">
  <SctiesFincgRptgMrgnDataTxStatRpt>
    <TradData>
      <Stat>
        <TechRcrdId>REC-NTRL</TechRcrdId>
        <RptgDtTm>2026-05-13T08:00:00Z</RptgDtTm>
        <EvtDt>2026-05-12</EvtDt>
        <CtrPty>
          <RptgCtrPty><Id><LEI>RPTGCPARTY0000000001</LEI></Id></RptgCtrPty>
          <OthrCtrPty><Id><Ntrl><Id>NATURAL-PERSON-ID-42</Id></Ntrl></Id></OthrCtrPty>
        </CtrPty>
        <CollPrtflId>PORTFOLIO-NTRL</CollPrtflId>
        <CtrctMod><ActnTp>NEWT</ActnTp></CtrctMod>
      </Stat>
    </TradData>
  </SctiesFincgRptgMrgnDataTxStatRpt>
</Document>"#;

    #[test]
    fn other_counterparty_natural_person_id_falls_through_lei_path() {
        // ESMA XSD allows OthrCtrPty to be either Lgl/LEI OR Ntrl/Id/Id.
        // We capture both into `other_counterparty` (string field) so
        // downstream LEI-format checks can flag the non-LEI case.
        let p = write_tmp("ntrl.xml", NATURAL_PERSON_OTHRCTRPTY);
        let outcome = read_sftr_margin_state_xml(&p).expect("parse");
        assert_eq!(outcome.records.len(), 1);
        let r = &outcome.records[0];
        assert_eq!(
            r.other_counterparty.as_deref(),
            Some("NATURAL-PERSON-ID-42")
        );
        let _ = std::fs::remove_file(&p);
    }
}
