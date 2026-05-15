//! SFTR Trade State Report (TSR) ingestion — ISO 20022 `auth.079`.
//!
//! Element paths are aligned with the real ESMA SFTR usage guideline
//! `auth.079.001.02_ESMAUG_SFTTRS_1.1.0`
//! (`SecuritiesFinancingReportingTransactionStateReportV02`). The
//! SWIFT-licensed XSD is **not** redistributed; only the schema *shape*
//! (element names, nesting, cardinalities) is encoded here. Coverage is
//! intentionally a documented subset — see
//! `docs/auth-messages/sftr-auth079.md`.
//!
//! Real envelope:
//! ```text
//! Document
//! └─ SctiesFincgRptgTxStatRpt    (SecuritiesFinancingReportingTransactionStateReportV02)
//!    └─ TradData  (choice)
//!       ├─ DataSetActn = "NOTX"  (empty / no-activity report)
//!       └─ Stat  (1..n)          (TradeStateReport16)
//!          ├─ TechRcrdId
//!          ├─ CtrPtySpcfcData
//!          │  ├─ RptgDtTm                       (per-record "state as of")
//!          │  └─ CtrPty/RptgCtrPty/Id/LEI , CtrPty/OthrCtrPty/Id/Lgl/LEI
//!          ├─ LnData  (4-way choice — wrapper = SFT type)
//!          │  ├─ RpTrad|BuySellBck : UnqTradIdr, EvtDt, ValDt,
//!          │  │      Term/Fxd/MtrtyDt, TermntnDt, PrncplAmt/ValDtAmt(@Ccy)
//!          │  ├─ SctiesLndg        : … LnVal(@Ccy)
//!          │  └─ MrgnLndg          : … OutsdngMrgnLnAmt(@Ccy)
//!          ├─ CollData  (4-way choice; Security52/55 components)
//!          │  └─ …/MktVal/Amt(@Ccy) , …/HrcutOrMrgn , …/Id=ISIN ,
//!          │     …/AvlblForCollReuse
//!          └─ CtrctMod/ActnTp
//! ```
//! `auth.079` has no header and no per-trade status element: a record
//! present in a Trade State Report *is* outstanding, so `status` is
//! `None` (the checks treat `None` as outstanding). SFT type is derived
//! from the `TransactionLoanData31Choice` wrapper. The single mapping
//! point is `commit_leaf`.

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
    /// File-level data-quality / parse issues (format / namespace).
    pub issues: Vec<DqIssue>,
}

const ISO20022_AUTH_079_NS: &[u8] = b"urn:iso:std:iso:20022:tech:xsd:auth.079.001.02";
/// The repeating per-trade record element under `TradData`.
const STAT_BLOCK: &str = "Stat";

/// Read an SFTR `auth.079` Trade State Report file.
pub fn read_sftr_tr_state_xml(path: &Path) -> anyhow::Result<SftrTrStateXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();

    if let Err(err) = check_wellformedness(path) {
        return Ok(SftrTrStateXmlReadOutcome {
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
        Some(ns) if ns == ISO20022_AUTH_079_NS => parse(path),
        other => {
            let actual = other
                .as_deref()
                .map(|n| String::from_utf8_lossy(n).into_owned())
                .unwrap_or_else(|| "(none)".into());
            Ok(SftrTrStateXmlReadOutcome {
                records: vec![],
                issues: vec![fmt_issue(
                    "SFTR.FMT.XML_UNSUPPORTED_NAMESPACE",
                    Severity::Warning,
                    format!(
                        "Root namespace is '{actual}', expected 'urn:iso:std:iso:20022:tech:xsd:auth.079.001.02'."
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

fn parse(path: &Path) -> anyhow::Result<SftrTrStateXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();
    let mut reader = NsReader::from_file(path)?;
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut pile: Vec<String> = Vec::new();
    let mut is_leaf: Vec<bool> = Vec::new();
    let mut text_buf = String::new();
    let mut attrs_buf: Vec<(String, String)> = Vec::new();

    let mut current: Option<SftrTrStateRecord> = None;
    let mut rec_depth: Option<usize> = None;
    let mut records: Vec<SftrTrStateRecord> = Vec::new();
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
                    current = Some(SftrTrStateRecord {
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
            "SFTR.FMT.SFTR_TSR_NO_RECORDS",
            Severity::Info,
            "SFTR Trade State Report carries TradData/DataSetActn \
             (no-activity report); zero state records to evaluate."
                .to_string(),
            source_label,
        ));
    }

    Ok(SftrTrStateXmlReadOutcome { records, issues })
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
/// `SftrTrStateRecord`. Real `auth.079.001.02` element paths; every
/// other branch is intentionally not extracted (documented in
/// `docs/auth-messages/sftr-auth079.md`).
fn commit_leaf(
    rec: &mut SftrTrStateRecord,
    rel: &[String],
    value: &str,
    attrs: &[(String, String)],
) {
    if value.is_empty() && attrs.is_empty() {
        return;
    }
    let record_id = rec.record_id.clone().unwrap_or_default();

    // SFT type from the TransactionLoanData31Choice / collateral
    // wrapper (no free SftTp element in auth.079).
    if rec.sft_type.is_none() {
        if has(rel, "RpTrad") {
            rec.sft_type = Some("REPO".to_owned());
        } else if has(rel, "BuySellBck") {
            rec.sft_type = Some("BSBC".to_owned());
        } else if has(rel, "SctiesLndg") {
            rec.sft_type = Some("SLEB".to_owned());
        } else if has(rel, "MrgnLndg") {
            rec.sft_type = Some("MGLD".to_owned());
        }
    }

    if tail(rel, &["UnqTradIdr"]) {
        if rec.uti.is_none() && !value.is_empty() {
            rec.uti = Some(value.to_owned());
        }
        return;
    }
    if tail(rel, &["LEI"]) && has(rel, "RptgCtrPty") {
        rec.reporting_counterparty = Some(value.to_owned());
        return;
    }
    if tail(rel, &["LEI"]) && has(rel, "OthrCtrPty") {
        rec.other_counterparty = Some(value.to_owned());
        return;
    }
    if rel.last().map(String::as_str) == Some("RptgDtTm") && has(rel, "CtrPtySpcfcData") {
        let mut tmp = None;
        set_dt(&mut tmp, value, "RptgDtTm", &record_id);
        if tmp.is_some() {
            rec.state_as_of = tmp;
        }
        return;
    }
    // Loan principal: PrncplAmt/ValDtAmt (repo/BSB) | LnVal (SecLn) |
    // OutsdngMrgnLnAmt (MgnLn). Each carries a required @Ccy.
    if tail(rel, &["PrncplAmt", "ValDtAmt"])
        || tail(rel, &["LnVal"])
        || tail(rel, &["OutsdngMrgnLnAmt"])
    {
        set_decimal(&mut rec.loan_value, value, "loan_value", &record_id);
        if let Some(ccy) = attr_of(attrs, "Ccy") {
            rec.loan_currency = Some(ccy.to_owned());
        }
        return;
    }
    if rel.last().map(String::as_str) == Some("EvtDt") {
        set_date(&mut rec.effective_date, value, "EvtDt", &record_id);
        return;
    }
    if rel.last().map(String::as_str) == Some("ValDt") {
        set_date(&mut rec.settlement_date, value, "ValDt", &record_id);
        return;
    }
    if tail(rel, &["Term", "Fxd", "MtrtyDt"]) {
        set_date(&mut rec.maturity_date, value, "MtrtyDt", &record_id);
        return;
    }
    if rel.last().map(String::as_str) == Some("TermntnDt") {
        set_date(&mut rec.termination_date, value, "TermntnDt", &record_id);
        return;
    }
    // Collateral component (Security52 for repo/BSB, Security55 for
    // MrgnLndg): first/representative component only.
    if tail(rel, &["MktVal", "Amt"]) && has(rel, "CollData") {
        set_decimal(&mut rec.collateral_value, value, "MktVal/Amt", &record_id);
        if let Some(ccy) = attr_of(attrs, "Ccy") {
            rec.collateral_currency = Some(ccy.to_owned());
        }
        return;
    }
    if rel.last().map(String::as_str) == Some("HrcutOrMrgn") {
        set_decimal(&mut rec.haircut, value, "HrcutOrMrgn", &record_id);
        return;
    }
    if rel.last().map(String::as_str) == Some("Id") && has(rel, "CollData") {
        if rec.collateral_isin.is_none() {
            rec.collateral_isin = Some(value.to_owned());
        }
        return;
    }
    if rel.last().map(String::as_str) == Some("AvlblForCollReuse") {
        if rec.reuse_indicator.is_none() {
            rec.reuse_indicator = match value.to_ascii_lowercase().as_str() {
                "true" | "1" | "y" | "yes" => Some(true),
                "false" | "0" | "n" | "no" => Some(false),
                _ => None,
            };
        }
        return;
    }
    // collateral_portfolio_code: auth.079 carries no SFT collateral
    // portfolio code at record level (the only PrtflCd is
    // clearing-specific) → left None. CtrctMod/ActnTp, TechRcrdId,
    // RcncltnFlg, MtrtyDtAmt, second leg, cash/commodity collateral and
    // everything else are preserved verbatim.
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
            "opendqi-sftr-tr-state-{}-{name}",
            std::process::id()
        ));
        std::fs::write(&p, content).unwrap();
        p
    }

    const REAL_ENVELOPE: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.079.001.02">
  <SctiesFincgRptgTxStatRpt>
    <TradData>
      <Stat>
        <TechRcrdId>REC-1</TechRcrdId>
        <CtrPtySpcfcData>
          <RptgDtTm>2026-05-13T08:00:00Z</RptgDtTm>
          <CtrPty>
            <RptgCtrPty><Id><LEI>RPTGCPARTY0000000001</LEI></Id></RptgCtrPty>
            <OthrCtrPty><Id><Lgl><LEI>OTHRCPARTY0000000002</LEI></Lgl></Id></OthrCtrPty>
          </CtrPty>
        </CtrPtySpcfcData>
        <LnData>
          <RpTrad>
            <UnqTradIdr>SFTR-TSR-S1</UnqTradIdr>
            <EvtDt>2026-05-12</EvtDt>
            <ValDt>2026-05-13</ValDt>
            <Term><Fxd><MtrtyDt>2030-01-01</MtrtyDt></Fxd></Term>
            <PrncplAmt><ValDtAmt Ccy="EUR">1000000.00</ValDtAmt></PrncplAmt>
          </RpTrad>
        </LnData>
        <CollData>
          <RpTrad><AsstTp><Scty>
            <Id>DE0001135275</Id>
            <MktVal><Amt Ccy="EUR">1050000.00</Amt></MktVal>
            <HrcutOrMrgn>0.05</HrcutOrMrgn>
            <AvlblForCollReuse>true</AvlblForCollReuse>
          </Scty></AsstTp></RpTrad>
        </CollData>
        <CtrctMod><ActnTp>NEWT</ActnTp></CtrctMod>
      </Stat>
      <Stat>
        <CtrPtySpcfcData><RptgDtTm>2026-05-13T08:00:00Z</RptgDtTm></CtrPtySpcfcData>
        <LnData><MrgnLndg>
          <UnqTradIdr>SFTR-TSR-S2</UnqTradIdr>
          <OutsdngMrgnLnAmt Ccy="USD">500000.00</OutsdngMrgnLnAmt>
        </MrgnLndg></LnData>
      </Stat>
    </TradData>
  </SctiesFincgRptgTxStatRpt>
</Document>"#;

    #[test]
    fn parses_real_auth079_envelope() {
        let p = write_tmp("real.xml", REAL_ENVELOPE);
        let out = read_sftr_tr_state_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert!(out.issues.is_empty());
        assert_eq!(out.records.len(), 2);
        let r0 = &out.records[0];
        assert_eq!(r0.uti.as_deref(), Some("SFTR-TSR-S1"));
        assert_eq!(r0.sft_type.as_deref(), Some("REPO"));
        assert_eq!(
            r0.reporting_counterparty.as_deref(),
            Some("RPTGCPARTY0000000001")
        );
        assert_eq!(
            r0.other_counterparty.as_deref(),
            Some("OTHRCPARTY0000000002")
        );
        assert_eq!(r0.loan_value.unwrap().to_string(), "1000000.00");
        assert_eq!(r0.loan_currency.as_deref(), Some("EUR"));
        assert_eq!(r0.collateral_value.unwrap().to_string(), "1050000.00");
        assert_eq!(r0.collateral_currency.as_deref(), Some("EUR"));
        assert_eq!(r0.haircut.unwrap().to_string(), "0.05");
        assert_eq!(r0.collateral_isin.as_deref(), Some("DE0001135275"));
        assert_eq!(r0.reuse_indicator, Some(true));
        assert_eq!(r0.maturity_date.unwrap().to_string(), "2030-01-01");
        assert!(r0.state_as_of.is_some(), "state_as_of from RptgDtTm");
        assert!(r0.status.is_none(), "auth.079 has no status element");
        assert!(r0.collateral_portfolio_code.is_none());

        let r1 = &out.records[1];
        assert_eq!(r1.sft_type.as_deref(), Some("MGLD"));
        assert_eq!(r1.loan_value.unwrap().to_string(), "500000.00");
        assert_eq!(r1.loan_currency.as_deref(), Some("USD"));
    }

    #[test]
    fn empty_report_no_records_info() {
        let body = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.079.001.02">
  <SctiesFincgRptgTxStatRpt>
    <TradData><DataSetActn>NOTX</DataSetActn></TradData>
  </SctiesFincgRptgTxStatRpt>
</Document>"#;
        let p = write_tmp("empty.xml", body);
        let out = read_sftr_tr_state_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert!(out.records.is_empty());
        assert_eq!(out.issues.len(), 1);
        assert_eq!(out.issues[0].check_id, "SFTR.FMT.SFTR_TSR_NO_RECORDS");
        assert_eq!(out.issues[0].severity, Severity::Info);
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
