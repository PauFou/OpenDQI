//! SFTR Reused Collateral Data Transaction State Report
//! (ISO 20022 `auth.086`) ingestion.
//!
//! Element paths are aligned with the real ESMA SFTR usage
//! guideline `auth.086.001.02_ESMAUG_1.0.0`
//! (`SecuritiesFinancingReportingReusedCollateralDataTransactionStateReportV02`).
//! The SWIFT-licensed XSD is **not** redistributed; only the
//! schema *shape* (element names, nesting, cardinalities) is
//! encoded here. Coverage is intentionally a documented subset
//! — see `docs/auth-messages/sftr-auth086.md` (added in
//! Phase H).
//!
//! Real envelope:
//! ```text
//! Document
//! └─ SctiesFincgRptgReusdCollDataTxStatRpt   (SecuritiesFinancingReportingReusedCollateralDataTransactionStateReportV02)
//!    └─ TradData                              (choice)
//!       ├─ DataSetActn = "NOTX"               (empty / no-activity report)
//!       └─ Stat  (1..n)                       (ReuseDataReportCorrection15__1 — single shape)
//!          ├─ TechRcrdId                                    (Max140Text)
//!          ├─ CtrPty (CounterpartyData87__1)
//!          │  ├─ RptSubmitgNtty/<choice>/LEI                → report_submitting_entity
//!          │  ├─ RptgCtrPty/<choice>/LEI                    → reporting_counterparty
//!          │  └─ NttyRspnsblForRpt/<choice>/LEI             → raw_fields
//!          ├─ CollCmpnt [0..1] (CollateralType19__1)
//!          │  ├─ Scty[] (SecurityReuseData1)
//!          │  │  ├─ ISIN                                    → raw_fields
//!          │  │  └─ ReuseVal/<Actl | Estmtd>/Amt(@Ccy)      → summed into total_reuse_value
//!          │  └─ Csh   [0..1] (CashReuseData1)
//!          │     ├─ RinvstdCsh[]                            → raw_fields
//!          │     └─ CshRinvstmtRate                         → cash_reinvestment_rate
//!          ├─ EvtDay                                        (ISODate)
//!          ├─ RptgDtTm                                      (ISONormalisedDateTime)
//!          ├─ FndgSrc[] [0..n]                              → raw_fields
//!          └─ CtrctMod/ActnTp                               → action_type (typically "REUU")
//! ```
//!
//! Structural relationship to the sister parsers :
//! - `sftr_reuse_activity.rs` (auth.071) = event log (4-way
//!   action wrappers New/Err/Crrctn/CollReuseUpd).
//! - `sftr_reuse_state.rs` (this) = state snapshot (single
//!   `Stat` block with `CtrctMod/ActnTp` leaf, mirror of
//!   `sftr_margin_state.rs`'s envelope on auth.071's content).
//!
//! Same content extraction logic as auth.071 (`commit_leaf`
//! mirrors `sftr_reuse_activity::commit_leaf` almost verbatim)
//! but the envelope detection uses the auth.085 `Stat`-block
//! pattern (no wrapper layer, action_type from leaf).

use std::path::Path;
use std::str::FromStr;

use chrono::{DateTime, NaiveDate, Utc};
use opendqi_core::{DqDimension, DqIssue, Regime, Severity, SftrReuseStateRecord};
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::NsReader;
use rust_decimal::Decimal;
use tracing::warn;

use crate::wellformed::check_wellformedness;

/// Outcome of reading one SFTR auth.086 XML file.
#[derive(Debug, Default)]
pub struct SftrReuseStateXmlReadOutcome {
    /// Records extracted from the file.
    pub records: Vec<SftrReuseStateRecord>,
    /// File-level data-quality / parse issues (format / namespace).
    pub issues: Vec<DqIssue>,
}

const ISO20022_AUTH_086_NS: &[u8] = b"urn:iso:std:iso:20022:tech:xsd:auth.086.001.02";
/// The repeating per-record element under `TradData` in auth.086.
const STAT_BLOCK: &str = "Stat";

/// Read an SFTR `auth.086` Reused Collateral Data Transaction
/// State Report file.
pub fn read_sftr_reuse_state_xml(path: &Path) -> anyhow::Result<SftrReuseStateXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();

    if let Err(err) = check_wellformedness(path) {
        return Ok(SftrReuseStateXmlReadOutcome {
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
        Some(ns) if ns == ISO20022_AUTH_086_NS => parse(path),
        other => {
            let actual = other
                .as_deref()
                .map(|n| String::from_utf8_lossy(n).into_owned())
                .unwrap_or_else(|| "(none)".into());
            Ok(SftrReuseStateXmlReadOutcome {
                records: vec![],
                issues: vec![fmt_issue(
                    "SFTR.FMT.XML_UNSUPPORTED_NAMESPACE",
                    Severity::Warning,
                    format!(
                        "Root namespace is '{actual}', expected 'urn:iso:std:iso:20022:tech:xsd:auth.086.001.02'."
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

fn parse(path: &Path) -> anyhow::Result<SftrReuseStateXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();
    let mut reader = NsReader::from_file(path)?;
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut pile: Vec<String> = Vec::new();
    let mut is_leaf: Vec<bool> = Vec::new();
    let mut text_buf = String::new();
    let mut attrs_buf: Vec<(String, String)> = Vec::new();

    let mut current: Option<SftrReuseStateRecord> = None;
    let mut rec_depth: Option<usize> = None;
    let mut records: Vec<SftrReuseStateRecord> = Vec::new();
    let mut rec_index: u32 = 0;
    let mut saw_dataset_actn = false;

    loop {
        match reader.read_resolved_event_into(&mut buf)? {
            (_, Event::Start(e)) => {
                let local = local_name(&e);
                push_element(&mut pile, &mut is_leaf, local);
                text_buf.clear();
                attrs_buf = collect_attrs(&e);

                // Record begins at the Stat block (mirror of
                // auth.085 envelope detection, not auth.071's
                // wrapper layer).
                if current.is_none()
                    && pile.last().map(String::as_str) == Some(STAT_BLOCK)
                    && pile.iter().any(|s| s == "TradData")
                {
                    rec_index += 1;
                    current = Some(SftrReuseStateRecord {
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
            "SFTR.FMT.SFTR_REU_STATE_NO_RECORDS",
            Severity::Info,
            "SFTR Reused Collateral Data Transaction State Report \
             carries TradData/DataSetActn (no-activity report); zero \
             reuse state records to evaluate."
                .to_string(),
            source_label,
        ));
    }

    Ok(SftrReuseStateXmlReadOutcome { records, issues })
}

/// True when `rel` ends with `suffix` (element-name tail match).
fn tail(rel: &[String], suffix: &[&str]) -> bool {
    rel.len() >= suffix.len()
        && rel[rel.len() - suffix.len()..]
            .iter()
            .zip(suffix)
            .all(|(a, b)| a == *b)
}

/// True when `seg` appears anywhere in `rel`.
fn has(rel: &[String], seg: &str) -> bool {
    rel.iter().any(|s| s == seg)
}

fn capture_ccy(rec: &mut SftrReuseStateRecord, attrs: &[(String, String)]) {
    if rec.reuse_currency.is_none() {
        if let Some(ccy) = attr_of(attrs, "Ccy") {
            rec.reuse_currency = Some(ccy.to_owned());
        }
    }
}

fn accumulate_reuse_value(rec: &mut SftrReuseStateRecord, value: &str, record: &str) {
    if value.is_empty() {
        return;
    }
    match Decimal::from_str(value) {
        Ok(v) => {
            rec.total_reuse_value = Some(rec.total_reuse_value.unwrap_or(Decimal::ZERO) + v);
        }
        Err(e) => {
            warn!(record = %record, field = "reuse_value", value = value, error = %e, "could not parse decimal")
        }
    }
}

/// Map one leaf (path relative to the Stat block) onto the
/// canonical SftrReuseStateRecord. Mirror of the auth.071
/// commit_leaf almost verbatim, with the addition of
/// `CtrctMod/ActnTp` extraction (the state-block uses a leaf
/// for action_type instead of a wrapper element).
fn commit_leaf(
    rec: &mut SftrReuseStateRecord,
    rel: &[String],
    value: &str,
    attrs: &[(String, String)],
) {
    if value.is_empty() && attrs.is_empty() {
        return;
    }
    let record_id = rec.record_id.clone().unwrap_or_default();

    // Header fields.
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
    if rel.last().map(String::as_str) == Some("EvtDay") {
        set_date(&mut rec.event_day, value, "EvtDay", &record_id);
        return;
    }
    // CtrctMod/ActnTp — the state-block carries action_type as
    // a leaf, mirror of auth.085's pattern.
    if rel.last().map(String::as_str) == Some("ActnTp") && has(rel, "CtrctMod") {
        if !value.is_empty() {
            rec.action_type = Some(value.to_owned());
        }
        return;
    }

    // Counterparties — same 3-entity firm-side shape as auth.071.
    if tail(rel, &["LEI"]) && has(rel, "RptgCtrPty") {
        rec.reporting_counterparty = Some(value.to_owned());
        return;
    }
    if tail(rel, &["LEI"]) && has(rel, "RptSubmitgNtty") {
        rec.report_submitting_entity = Some(value.to_owned());
        return;
    }

    // CollCmpnt/Scty[]/ReuseVal/<choice>/Amt(@Ccy) — sum across,
    // promote first @Ccy onto reuse_currency.
    if tail(rel, &["Actl"]) && has(rel, "ReuseVal") && has(rel, "Scty") {
        accumulate_reuse_value(rec, value, &record_id);
        capture_ccy(rec, attrs);
        return;
    }
    if tail(rel, &["Estmtd"]) && has(rel, "ReuseVal") && has(rel, "Scty") {
        accumulate_reuse_value(rec, value, &record_id);
        capture_ccy(rec, attrs);
        return;
    }

    // CollCmpnt/Csh/CshRinvstmtRate — single Decimal percentage.
    if tail(rel, &["CshRinvstmtRate"]) && has(rel, "Csh") {
        let mut tmp = None;
        set_decimal(&mut tmp, value, "cash_reinvestment_rate", &record_id);
        if tmp.is_some() {
            rec.cash_reinvestment_rate = tmp;
        }
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
        let p = std::env::temp_dir().join(format!(
            "opendqi-sftr-reu-state-{}-{name}",
            std::process::id()
        ));
        std::fs::write(&p, content).unwrap();
        p
    }

    const FULL_STATE: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.086.001.02">
  <SctiesFincgRptgReusdCollDataTxStatRpt>
    <TradData>
      <Stat>
        <TechRcrdId>REUSE-STATE-1</TechRcrdId>
        <CtrPty>
          <RptSubmitgNtty><LEI>SUBMITRPT000000000001</LEI></RptSubmitgNtty>
          <RptgCtrPty><LEI>RPTGCPARTY0000000001</LEI></RptgCtrPty>
          <NttyRspnsblForRpt><LEI>ENTRESP00000000000001</LEI></NttyRspnsblForRpt>
        </CtrPty>
        <CollCmpnt>
          <Scty>
            <ISIN>DE000A1B2C34</ISIN>
            <ReuseVal><Actl Ccy="EUR">1000.50</Actl></ReuseVal>
          </Scty>
          <Csh>
            <RinvstdCsh><Tp>OTHR</Tp><Amt Ccy="EUR">250.00</Amt></RinvstdCsh>
            <CshRinvstmtRate>0.0125</CshRinvstmtRate>
          </Csh>
        </CollCmpnt>
        <EvtDay>2026-05-12</EvtDay>
        <RptgDtTm>2026-05-13T08:00:00Z</RptgDtTm>
        <FndgSrc><Tp>SECL</Tp><MktVal><Amt Ccy="EUR">10000.00</Amt></MktVal></FndgSrc>
        <CtrctMod><ActnTp>REUU</ActnTp></CtrctMod>
      </Stat>
    </TradData>
  </SctiesFincgRptgReusdCollDataTxStatRpt>
</Document>"#;

    #[test]
    fn parses_full_state_record_with_action_type_from_leaf() {
        let p = write_tmp("full.xml", FULL_STATE);
        let outcome = read_sftr_reuse_state_xml(&p).expect("parse");
        assert!(outcome.issues.is_empty(), "no format issues");
        assert_eq!(outcome.records.len(), 1);
        let r = &outcome.records[0];
        assert_eq!(r.record_id.as_deref(), Some("REUSE-STATE-1"));
        assert_eq!(r.regime, Regime::Sftr);
        // CtrctMod/ActnTp leaf (state-block convention) — auth.086
        // doesn't use the action-wrapper pattern of auth.071.
        assert_eq!(r.action_type.as_deref(), Some("REUU"));
        assert_eq!(
            r.reporting_counterparty.as_deref(),
            Some("RPTGCPARTY0000000001")
        );
        assert_eq!(
            r.report_submitting_entity.as_deref(),
            Some("SUBMITRPT000000000001")
        );
        assert_eq!(
            r.total_reuse_value,
            Some(Decimal::from_str("1000.50").unwrap())
        );
        assert_eq!(r.reuse_currency.as_deref(), Some("EUR"));
        assert_eq!(
            r.cash_reinvestment_rate,
            Some(Decimal::from_str("0.0125").unwrap())
        );
        assert!(r.event_day.is_some());
        assert!(r.state_as_of.is_some());
        let _ = std::fs::remove_file(&p);
    }

    const CASH_ONLY_STATE: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.086.001.02">
  <SctiesFincgRptgReusdCollDataTxStatRpt>
    <TradData>
      <Stat>
        <TechRcrdId>R-CASH</TechRcrdId>
        <CtrPty>
          <RptSubmitgNtty><LEI>LEI-S</LEI></RptSubmitgNtty>
          <RptgCtrPty><LEI>LEI-R</LEI></RptgCtrPty>
          <NttyRspnsblForRpt><LEI>LEI-E</LEI></NttyRspnsblForRpt>
        </CtrPty>
        <CollCmpnt>
          <Csh>
            <RinvstdCsh><Tp>FREE</Tp><Amt Ccy="USD">5000.00</Amt></RinvstdCsh>
            <CshRinvstmtRate>0.0200</CshRinvstmtRate>
          </Csh>
        </CollCmpnt>
        <EvtDay>2026-05-12</EvtDay>
        <RptgDtTm>2026-05-13T08:00:00Z</RptgDtTm>
        <CtrctMod><ActnTp>REUU</ActnTp></CtrctMod>
      </Stat>
    </TradData>
  </SctiesFincgRptgReusdCollDataTxStatRpt>
</Document>"#;

    #[test]
    fn cash_only_state_has_rate_set_but_no_total_reuse_value() {
        let p = write_tmp("cash.xml", CASH_ONLY_STATE);
        let outcome = read_sftr_reuse_state_xml(&p).expect("parse");
        assert_eq!(outcome.records.len(), 1);
        let r = &outcome.records[0];
        assert!(r.total_reuse_value.is_none());
        assert!(r.reuse_currency.is_none());
        assert_eq!(
            r.cash_reinvestment_rate,
            Some(Decimal::from_str("0.0200").unwrap())
        );
        let _ = std::fs::remove_file(&p);
    }

    const NOTX: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.086.001.02">
  <SctiesFincgRptgReusdCollDataTxStatRpt>
    <TradData>
      <DataSetActn>NOTX</DataSetActn>
    </TradData>
  </SctiesFincgRptgReusdCollDataTxStatRpt>
</Document>"#;

    #[test]
    fn no_records_with_dataset_actn_emits_info_issue() {
        let p = write_tmp("notx.xml", NOTX);
        let outcome = read_sftr_reuse_state_xml(&p).expect("parse");
        assert!(outcome.records.is_empty());
        assert_eq!(outcome.issues.len(), 1);
        assert_eq!(
            outcome.issues[0].check_id,
            "SFTR.FMT.SFTR_REU_STATE_NO_RECORDS"
        );
        let _ = std::fs::remove_file(&p);
    }

    const WRONG_NS: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.999.001.02"><X/></Document>"#;

    #[test]
    fn wrong_namespace_emits_warning_no_records() {
        let p = write_tmp("wrong_ns.xml", WRONG_NS);
        let outcome = read_sftr_reuse_state_xml(&p).expect("parse");
        assert!(outcome.records.is_empty());
        assert_eq!(outcome.issues.len(), 1);
        assert_eq!(
            outcome.issues[0].check_id,
            "SFTR.FMT.XML_UNSUPPORTED_NAMESPACE"
        );
        let _ = std::fs::remove_file(&p);
    }

    const MULTI_SCTY_SUM: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.086.001.02">
  <SctiesFincgRptgReusdCollDataTxStatRpt>
    <TradData>
      <Stat>
        <TechRcrdId>R-MULTI</TechRcrdId>
        <CtrPty>
          <RptSubmitgNtty><LEI>LEI-S</LEI></RptSubmitgNtty>
          <RptgCtrPty><LEI>LEI-R</LEI></RptgCtrPty>
          <NttyRspnsblForRpt><LEI>LEI-E</LEI></NttyRspnsblForRpt>
        </CtrPty>
        <CollCmpnt>
          <Scty>
            <ISIN>DE000AAA0001</ISIN>
            <ReuseVal><Actl Ccy="EUR">100.00</Actl></ReuseVal>
          </Scty>
          <Scty>
            <ISIN>DE000AAA0002</ISIN>
            <ReuseVal><Estmtd Ccy="EUR">200.00</Estmtd></ReuseVal>
          </Scty>
          <Scty>
            <ISIN>DE000AAA0003</ISIN>
            <ReuseVal><Actl Ccy="EUR">50.00</Actl></ReuseVal>
          </Scty>
        </CollCmpnt>
        <EvtDay>2026-05-12</EvtDay>
        <RptgDtTm>2026-05-13T08:00:00Z</RptgDtTm>
        <CtrctMod><ActnTp>REUU</ActnTp></CtrctMod>
      </Stat>
    </TradData>
  </SctiesFincgRptgReusdCollDataTxStatRpt>
</Document>"#;

    #[test]
    fn multi_scty_sums_actl_and_estmtd_uniformly() {
        let p = write_tmp("multi.xml", MULTI_SCTY_SUM);
        let outcome = read_sftr_reuse_state_xml(&p).expect("parse");
        let r = &outcome.records[0];
        // 100 + 200 + 50 = 350.
        assert_eq!(
            r.total_reuse_value,
            Some(Decimal::from_str("350.00").unwrap())
        );
        assert_eq!(r.reuse_currency.as_deref(), Some("EUR"));
        let _ = std::fs::remove_file(&p);
    }
}
