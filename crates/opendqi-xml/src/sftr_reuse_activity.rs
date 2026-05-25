//! SFTR Reused Collateral Data Report (ISO 20022 `auth.071`)
//! ingestion.
//!
//! Element paths are aligned with the real ESMA SFTR usage
//! guideline `auth.071.001.02_ESMAUG_1.1.0`
//! (`SecuritiesFinancingReportingTransactionReusedCollateralDataReportV02`).
//! The SWIFT-licensed XSD is **not** redistributed; only the
//! schema *shape* (element names, nesting, cardinalities) is
//! encoded here. Coverage is intentionally a documented subset
//! — see `docs/auth-messages/sftr-auth071.md` (added in
//! Phase H).
//!
//! Real envelope:
//! ```text
//! Document
//! └─ SctiesFincgRptgTxReusdCollDataRpt   (SecuritiesFinancingReportingTransactionReusedCollateralDataReportV02)
//!    └─ TradData                          (choice)
//!       ├─ DataSetActn = "NOTX"           (empty / no-activity report)
//!       └─ Rpt  (1..n)                    (ReuseDataReport6Choice__1 — action-typed wrapper)
//!          └─ <choice: New | Err | Crrctn | CollReuseUpd>
//!             ├─ TechRcrdId                              (Max140Text — optional in New, required elsewhere)
//!             ├─ RptgDtTm                                (ISODateTime, per-event "state as of")
//!             ├─ CtrPty (CounterpartyData87__1 or __2 in Err)
//!             │  ├─ RptSubmitgNtty/<choice>/LEI          → report_submitting_entity
//!             │  ├─ RptgCtrPty/<choice>/LEI              → reporting_counterparty
//!             │  └─ NttyRspnsblForRpt/<choice>/LEI       → raw_fields
//!             ├─ CollCmpnt  [0..1] (CollateralType19__1, absent in Err)
//!             │  ├─ Scty[]  (SecurityReuseData1)
//!             │  │  ├─ ISIN                              → raw_fields
//!             │  │  └─ ReuseVal/<choice: Actl | Estmtd>/Amt(@Ccy)   → summed into total_reuse_value
//!             │  └─ Csh   [0..1] (CashReuseData1)
//!             │     ├─ RinvstdCsh[]                      → raw_fields
//!             │     └─ CshRinvstmtRate                   → cash_reinvestment_rate
//!             ├─ EvtDay                                  (ISODate, absent in Err)
//!             └─ FndgSrc[]  [0..n] (FundingSource3)
//!                ├─ Tp (FundingSourceType1Code: SECL|FREE|OTHR) → raw_fields
//!                └─ MktVal/Amt(@Ccy)                            → raw_fields
//! ```
//!
//! Key differences vs auth.070 (the sister sftr_margin_activity
//! parser):
//! - no `OthrCtrPty` — auth.071 is firm-portfolio-level, not
//!   bilateral. 3 entities (RptSubmitgNtty / RptgCtrPty /
//!   NttyRspnsblForRpt), the first 2 promoted onto typed
//!   fields, the third left in raw_fields.
//! - no `CollPrtflId` — records keyed intrinsically by
//!   submitter + event day + ISIN.
//! - variable-count Scty[]/ReuseVal entries — the parser sums
//!   every observed amount (Actl or Estmtd, no discrimination)
//!   into `total_reuse_value` and promotes the first observed
//!   `@Ccy` onto `reuse_currency`. The per-ISIN breakdown is
//!   captured into `raw_fields`.
//! - action_type wrapper alphabet: `New`/`Err`/`Crrctn`/
//!   `CollReuseUpd` → `NEWT`/`ERRT`/`CORR`/`CRUD`.
//!
//! Sister parser: `sftr_margin_activity.rs` (auth.070 — margin
//! events).

use std::path::Path;
use std::str::FromStr;

use chrono::{DateTime, NaiveDate, Utc};
use opendqi_core::{DqDimension, DqIssue, Regime, Severity, SftrReuseActivityRecord};
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::NsReader;
use rust_decimal::Decimal;
use tracing::warn;

use crate::wellformed::check_wellformedness;

/// Outcome of reading one SFTR auth.071 XML file.
#[derive(Debug, Default)]
pub struct SftrReuseActivityXmlReadOutcome {
    /// Records extracted from the file.
    pub records: Vec<SftrReuseActivityRecord>,
    /// File-level data-quality / parse issues (format / namespace).
    pub issues: Vec<DqIssue>,
}

const ISO20022_AUTH_071_NS: &[u8] = b"urn:iso:std:iso:20022:tech:xsd:auth.071.001.02";
/// The repeating per-record element under `TradData` in auth.071.
const RPT_BLOCK: &str = "Rpt";

/// Read an SFTR `auth.071` Reused Collateral Data Report file.
pub fn read_sftr_reuse_activity_xml(
    path: &Path,
) -> anyhow::Result<SftrReuseActivityXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();

    if let Err(err) = check_wellformedness(path) {
        return Ok(SftrReuseActivityXmlReadOutcome {
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
        Some(ns) if ns == ISO20022_AUTH_071_NS => parse(path),
        other => {
            let actual = other
                .as_deref()
                .map(|n| String::from_utf8_lossy(n).into_owned())
                .unwrap_or_else(|| "(none)".into());
            Ok(SftrReuseActivityXmlReadOutcome {
                records: vec![],
                issues: vec![fmt_issue(
                    "SFTR.FMT.XML_UNSUPPORTED_NAMESPACE",
                    Severity::Warning,
                    format!(
                        "Root namespace is '{actual}', expected 'urn:iso:std:iso:20022:tech:xsd:auth.071.001.02'."
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

/// Map the wrapper element name to the canonical 4-letter
/// action_type code. Returns `None` when the element is not one
/// of the 4 known `ReuseDataReport6Choice__1` branches.
fn wrapper_action_type(local: &str) -> Option<&'static str> {
    match local {
        "New" => Some("NEWT"),
        "Err" => Some("ERRT"),
        "Crrctn" => Some("CORR"),
        "CollReuseUpd" => Some("CRUD"),
        _ => None,
    }
}

fn parse(path: &Path) -> anyhow::Result<SftrReuseActivityXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();
    let mut reader = NsReader::from_file(path)?;
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut pile: Vec<String> = Vec::new();
    let mut is_leaf: Vec<bool> = Vec::new();
    let mut text_buf = String::new();
    let mut attrs_buf: Vec<(String, String)> = Vec::new();

    let mut current: Option<SftrReuseActivityRecord> = None;
    let mut rec_depth: Option<usize> = None;
    let mut records: Vec<SftrReuseActivityRecord> = Vec::new();
    let mut rec_index: u32 = 0;
    let mut saw_dataset_actn = false;

    loop {
        match reader.read_resolved_event_into(&mut buf)? {
            (_, Event::Start(e)) => {
                let local = local_name(&e);
                push_element(&mut pile, &mut is_leaf, local.clone());
                text_buf.clear();
                attrs_buf = collect_attrs(&e);

                // Record begins at the *wrapper* depth (1 level
                // under `Rpt`). Mirror of the auth.070 strategy.
                if current.is_none()
                    && pile.len() >= 2
                    && pile[pile.len() - 2] == RPT_BLOCK
                    && pile.iter().any(|s| s == "TradData")
                {
                    if let Some(actn) = wrapper_action_type(&local) {
                        rec_index += 1;
                        current = Some(SftrReuseActivityRecord {
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
            "SFTR.FMT.SFTR_REU_NO_RECORDS",
            Severity::Info,
            "SFTR Reused Collateral Data Report carries \
             TradData/DataSetActn (no-activity report); zero \
             reuse activity records to evaluate."
                .to_string(),
            source_label,
        ));
    }

    Ok(SftrReuseActivityXmlReadOutcome { records, issues })
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

/// Promote the per-amount `@Ccy` attribute onto `reuse_currency`
/// the first time we see one on a reuse-value amount. Cash-side
/// MktVal currencies stay in raw_fields to avoid drift.
fn capture_ccy(rec: &mut SftrReuseActivityRecord, attrs: &[(String, String)]) {
    if rec.reuse_currency.is_none() {
        if let Some(ccy) = attr_of(attrs, "Ccy") {
            rec.reuse_currency = Some(ccy.to_owned());
        }
    }
}

/// Accumulate `value` (as Decimal) into `total_reuse_value` —
/// sums across every Scty/ReuseVal/{Actl|Estmtd} entry observed.
fn accumulate_reuse_value(rec: &mut SftrReuseActivityRecord, value: &str, record: &str) {
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

/// Map one leaf (path relative to the wrapper element) onto the
/// canonical `SftrReuseActivityRecord`. Real `auth.071.001.02`
/// element paths; every other branch is captured into
/// `raw_fields`.
fn commit_leaf(
    rec: &mut SftrReuseActivityRecord,
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

    // Counterparties. The 3-entity shape under CtrPty:
    //   CtrPty/{RptSubmitgNtty | RptgCtrPty | NttyRspnsblForRpt}/<choice>/LEI
    //   or                                                      /<choice>/Ntrl/Id/Id
    if tail(rel, &["LEI"]) && has(rel, "RptgCtrPty") {
        rec.reporting_counterparty = Some(value.to_owned());
        return;
    }
    if tail(rel, &["LEI"]) && has(rel, "RptSubmitgNtty") {
        rec.report_submitting_entity = Some(value.to_owned());
        return;
    }
    // NttyRspnsblForRpt + any natural-person id paths fall
    // through to raw_fields.

    // CollCmpnt/Scty[]/ReuseVal/<choice>/Amt(@Ccy) — sum across
    // every entry, promote the first observed @Ccy onto
    // reuse_currency.
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
        let p =
            std::env::temp_dir().join(format!("opendqi-sftr-reu-{}-{name}", std::process::id()));
        std::fs::write(&p, content).unwrap();
        p
    }

    const NEW_FULL_RECORD: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.071.001.02">
  <SctiesFincgRptgTxReusdCollDataRpt>
    <TradData>
      <Rpt>
        <New>
          <TechRcrdId>REUSE-NEW-1</TechRcrdId>
          <RptgDtTm>2026-05-13T08:00:00Z</RptgDtTm>
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
            <Scty>
              <ISIN>DE000A1B2C35</ISIN>
              <ReuseVal><Estmtd Ccy="EUR">500.00</Estmtd></ReuseVal>
            </Scty>
            <Csh>
              <RinvstdCsh><Tp>OTHR</Tp><Amt Ccy="EUR">250.00</Amt></RinvstdCsh>
              <CshRinvstmtRate>0.0125</CshRinvstmtRate>
            </Csh>
          </CollCmpnt>
          <EvtDay>2026-05-12</EvtDay>
          <FndgSrc><Tp>SECL</Tp><MktVal><Amt Ccy="EUR">10000.00</Amt></MktVal></FndgSrc>
        </New>
      </Rpt>
    </TradData>
  </SctiesFincgRptgTxReusdCollDataRpt>
</Document>"#;

    #[test]
    fn parses_new_wrapper_full_record_total_reuse_value_sums_actl_and_estmtd() {
        let p = write_tmp("new_full.xml", NEW_FULL_RECORD);
        let outcome = read_sftr_reuse_activity_xml(&p).expect("parse");
        assert!(
            outcome.issues.is_empty(),
            "no format issues expected: {:?}",
            outcome.issues
        );
        assert_eq!(outcome.records.len(), 1);
        let r = &outcome.records[0];
        assert_eq!(r.record_id.as_deref(), Some("REUSE-NEW-1"));
        assert_eq!(r.regime, Regime::Sftr);
        assert_eq!(r.action_type.as_deref(), Some("NEWT"));
        assert_eq!(
            r.reporting_counterparty.as_deref(),
            Some("RPTGCPARTY0000000001")
        );
        assert_eq!(
            r.report_submitting_entity.as_deref(),
            Some("SUBMITRPT000000000001")
        );
        // 1000.50 (Actl) + 500.00 (Estmtd) = 1500.50.
        assert_eq!(
            r.total_reuse_value,
            Some(Decimal::from_str("1500.50").unwrap())
        );
        assert_eq!(r.reuse_currency.as_deref(), Some("EUR"));
        assert_eq!(
            r.cash_reinvestment_rate,
            Some(Decimal::from_str("0.0125").unwrap())
        );
        assert_eq!(
            r.event_day,
            Some(NaiveDate::from_ymd_opt(2026, 5, 12).unwrap())
        );
        // NttyRspnsblForRpt + last-seen ISIN + FndgSrc captured
        // into raw_fields. The two Scty entries collide on
        // path `CollCmpnt/Scty/ISIN` so the BTreeMap keeps the
        // last value (this matches the raw_fields convention
        // adopted across the 6 other SFTR parsers — the DQI
        // computers and granular checks all consume typed
        // fields only, so the collision is benign).
        assert!(r.raw_fields.values().any(|v| v == "ENTRESP00000000000001"));
        assert!(
            r.raw_fields.values().any(|v| v == "DE000A1B2C35"),
            "last-seen ISIN should win in raw_fields: {:?}",
            r.raw_fields
        );
        assert!(r.raw_fields.values().any(|v| v == "SECL"));
        let _ = std::fs::remove_file(&p);
    }

    const ALL_FOUR_WRAPPERS: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.071.001.02">
  <SctiesFincgRptgTxReusdCollDataRpt>
    <TradData>
      <Rpt><New>
        <TechRcrdId>R-NEW</TechRcrdId>
        <RptgDtTm>2026-05-13T08:00:00Z</RptgDtTm>
        <CtrPty>
          <RptSubmitgNtty><LEI>LEI-S</LEI></RptSubmitgNtty>
          <RptgCtrPty><LEI>LEI-R</LEI></RptgCtrPty>
          <NttyRspnsblForRpt><LEI>LEI-E</LEI></NttyRspnsblForRpt>
        </CtrPty>
        <EvtDay>2026-05-12</EvtDay>
      </New></Rpt>
      <Rpt><Crrctn>
        <TechRcrdId>R-CORR</TechRcrdId>
        <RptgDtTm>2026-05-13T08:00:00Z</RptgDtTm>
        <CtrPty>
          <RptSubmitgNtty><LEI>LEI-S</LEI></RptSubmitgNtty>
          <RptgCtrPty><LEI>LEI-R</LEI></RptgCtrPty>
          <NttyRspnsblForRpt><LEI>LEI-E</LEI></NttyRspnsblForRpt>
        </CtrPty>
        <EvtDay>2026-05-12</EvtDay>
      </Crrctn></Rpt>
      <Rpt><CollReuseUpd>
        <TechRcrdId>R-CRUD</TechRcrdId>
        <RptgDtTm>2026-05-13T08:00:00Z</RptgDtTm>
        <CtrPty>
          <RptSubmitgNtty><LEI>LEI-S</LEI></RptSubmitgNtty>
          <RptgCtrPty><LEI>LEI-R</LEI></RptgCtrPty>
          <NttyRspnsblForRpt><LEI>LEI-E</LEI></NttyRspnsblForRpt>
        </CtrPty>
        <EvtDay>2026-05-12</EvtDay>
      </CollReuseUpd></Rpt>
      <Rpt><Err>
        <TechRcrdId>R-ERR</TechRcrdId>
        <RptgDtTm>2026-05-13T08:00:00Z</RptgDtTm>
        <CtrPty>
          <RptSubmitgNtty><LEI>LEI-S</LEI></RptSubmitgNtty>
          <RptgCtrPty><LEI>LEI-R</LEI></RptgCtrPty>
          <NttyRspnsblForRpt><LEI>LEI-E</LEI></NttyRspnsblForRpt>
        </CtrPty>
      </Err></Rpt>
    </TradData>
  </SctiesFincgRptgTxReusdCollDataRpt>
</Document>"#;

    #[test]
    fn four_wrappers_map_to_canonical_action_codes_err_has_no_evtday() {
        let p = write_tmp("four_wrappers.xml", ALL_FOUR_WRAPPERS);
        let outcome = read_sftr_reuse_activity_xml(&p).expect("parse");
        assert!(outcome.issues.is_empty());
        assert_eq!(outcome.records.len(), 4);
        let actions: Vec<&str> = outcome
            .records
            .iter()
            .map(|r| r.action_type.as_deref().unwrap())
            .collect();
        assert_eq!(actions, vec!["NEWT", "CORR", "CRUD", "ERRT"]);
        let err = outcome
            .records
            .iter()
            .find(|r| r.action_type.as_deref() == Some("ERRT"))
            .unwrap();
        assert!(err.event_day.is_none(), "Err wrapper has no EvtDay");
        for r in outcome
            .records
            .iter()
            .filter(|r| r.action_type.as_deref() != Some("ERRT"))
        {
            assert!(r.event_day.is_some(), "non-Err wrappers have EvtDay");
        }
        let _ = std::fs::remove_file(&p);
    }

    const CASH_ONLY: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.071.001.02">
  <SctiesFincgRptgTxReusdCollDataRpt>
    <TradData>
      <Rpt><New>
        <TechRcrdId>R-CASH</TechRcrdId>
        <RptgDtTm>2026-05-13T08:00:00Z</RptgDtTm>
        <CtrPty>
          <RptSubmitgNtty><LEI>LEI-S</LEI></RptSubmitgNtty>
          <RptgCtrPty><LEI>LEI-R</LEI></RptgCtrPty>
          <NttyRspnsblForRpt><LEI>LEI-E</LEI></NttyRspnsblForRpt>
        </CtrPty>
        <CollCmpnt>
          <Csh>
            <RinvstdCsh><Tp>OTHR</Tp><Amt Ccy="USD">5000.00</Amt></RinvstdCsh>
            <CshRinvstmtRate>0.0200</CshRinvstmtRate>
          </Csh>
        </CollCmpnt>
        <EvtDay>2026-05-12</EvtDay>
      </New></Rpt>
    </TradData>
  </SctiesFincgRptgTxReusdCollDataRpt>
</Document>"#;

    #[test]
    fn cash_only_record_has_rate_set_but_no_total_reuse_value() {
        // Pure cash reinvestment (no Scty entries): the
        // total_reuse_value stays None, only the rate is set.
        let p = write_tmp("cash_only.xml", CASH_ONLY);
        let outcome = read_sftr_reuse_activity_xml(&p).expect("parse");
        assert_eq!(outcome.records.len(), 1);
        let r = &outcome.records[0];
        assert!(r.total_reuse_value.is_none());
        // reuse_currency stays None (only set by Scty/ReuseVal paths).
        assert!(r.reuse_currency.is_none());
        assert_eq!(
            r.cash_reinvestment_rate,
            Some(Decimal::from_str("0.0200").unwrap())
        );
        let _ = std::fs::remove_file(&p);
    }

    const NOTX: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.071.001.02">
  <SctiesFincgRptgTxReusdCollDataRpt>
    <TradData>
      <DataSetActn>NOTX</DataSetActn>
    </TradData>
  </SctiesFincgRptgTxReusdCollDataRpt>
</Document>"#;

    #[test]
    fn no_records_with_dataset_actn_emits_info_issue() {
        let p = write_tmp("notx.xml", NOTX);
        let outcome = read_sftr_reuse_activity_xml(&p).expect("parse");
        assert!(outcome.records.is_empty());
        assert_eq!(outcome.issues.len(), 1);
        assert_eq!(outcome.issues[0].check_id, "SFTR.FMT.SFTR_REU_NO_RECORDS");
        let _ = std::fs::remove_file(&p);
    }

    const WRONG_NS: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.999.001.02"><X/></Document>"#;

    #[test]
    fn wrong_namespace_emits_warning_no_records() {
        let p = write_tmp("wrong_ns.xml", WRONG_NS);
        let outcome = read_sftr_reuse_activity_xml(&p).expect("parse");
        assert!(outcome.records.is_empty());
        assert_eq!(outcome.issues.len(), 1);
        assert_eq!(
            outcome.issues[0].check_id,
            "SFTR.FMT.XML_UNSUPPORTED_NAMESPACE"
        );
        let _ = std::fs::remove_file(&p);
    }
}
