//! EMIR Derivatives Trade Position Set Report (ISO 20022
//! `auth.090`) ingestion.
//!
//! Element paths are aligned with
//! `auth.090.001.02_ESMAUG_DATPOS_1.0.0`
//! (`DerivativesTradePositionSetReportV02`). The SWIFT-licensed
//! XSD is **not** redistributed; only the schema *shape*
//! (element names, nesting, cardinalities) is encoded here.
//!
//! Real envelope:
//! ```text
//! Document
//! └─ DerivsTradPosSetRpt
//!    └─ AggtdPos  (PositionSetAggregated2Choice__1)
//!       ├─ DataSetActn = "NOTX"
//!       └─ Rpt    (PositionSetAggregated4__1)
//!          ├─ RefDt                          (ISODate, shared across the file)
//!          ├─ PosSet[]        (up to 500_000)
//!          ├─ CcyPosSet[]
//!          ├─ CollPosSet[]
//!          └─ CcyCollPosSet[]
//! ```
//!
//! Each PosSet/CcyPosSet/CollPosSet/CcyCollPosSet has:
//! ```text
//! ├─ Dmnsns (PositionSetDimensions16__1)
//! │  ├─ CtrPtyId/RptgCtrPty/.../LEI
//! │  ├─ CtrPtyId/OthrCtrPty/.../LEI
//! │  ├─ ValCcy (ISO 4217)
//! │  ├─ AsstClss (ProductType4Code)
//! │  ├─ CtrctTp (FinancialInstrumentContractType2Code)
//! │  └─ UndrlygInstrm/.../ISIN
//! └─ Mtrcs
//!    ├─ Ttl/Buyr/Ntnl/Amt           (notional, PosSet kinds)
//!    ├─ Ttl/Buyr/PostvVal           (MtM-ish, PosSet kinds)
//!    └─ Ttl/PstdMrgnOrColl/.../Amt  (collateral, CollPosSet kinds)
//! ```
//!
//! Honest scope: auth.090 is the largest XSD we parse (5400 L)
//! and has rich buyer/seller + total/clean splits we don't
//! fully expose. The parser captures the DQ-actionable headline
//! numbers (one Decimal each) and leaves the rest in raw_fields
//! for downstream inspection.

use std::path::Path;
use std::str::FromStr;

use chrono::NaiveDate;
use opendqi_core::{DqDimension, DqIssue, EmirPositionSetRecord, Regime, Severity};
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::NsReader;
use rust_decimal::Decimal;
use tracing::warn;

use crate::wellformed::check_wellformedness;

/// Outcome of reading one EMIR auth.090 XML file.
#[derive(Debug, Default)]
pub struct EmirPositionSetXmlReadOutcome {
    /// Records extracted from the file (1 per PosSet / CcyPosSet
    /// / CollPosSet / CcyCollPosSet element).
    pub records: Vec<EmirPositionSetRecord>,
    /// File-level data-quality / parse issues.
    pub issues: Vec<DqIssue>,
}

const ISO20022_AUTH_090_NS: &[u8] = b"urn:iso:std:iso:20022:tech:xsd:auth.090.001.02";

/// The 4 position-set wrapper names under `Rpt`.
fn wrapper_kind(local: &str) -> Option<&'static str> {
    match local {
        "PosSet" => Some("PosSet"),
        "CcyPosSet" => Some("CcyPosSet"),
        "CollPosSet" => Some("CollPosSet"),
        "CcyCollPosSet" => Some("CcyCollPosSet"),
        _ => None,
    }
}

/// Read an EMIR `auth.090` Position Set Report file.
pub fn read_emir_position_set_xml(path: &Path) -> anyhow::Result<EmirPositionSetXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();

    if let Err(err) = check_wellformedness(path) {
        return Ok(EmirPositionSetXmlReadOutcome {
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
        Some(ns) if ns == ISO20022_AUTH_090_NS => parse(path),
        other => {
            let actual = other
                .as_deref()
                .map(|n| String::from_utf8_lossy(n).into_owned())
                .unwrap_or_else(|| "(none)".into());
            Ok(EmirPositionSetXmlReadOutcome {
                records: vec![],
                issues: vec![fmt_issue(
                    "EMIR.FMT.XML_UNSUPPORTED_NAMESPACE",
                    Severity::Warning,
                    format!(
                        "Root namespace is '{actual}', expected 'urn:iso:std:iso:20022:tech:xsd:auth.090.001.02'."
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

fn parse(path: &Path) -> anyhow::Result<EmirPositionSetXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();
    let mut reader = NsReader::from_file(path)?;
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut pile: Vec<String> = Vec::new();
    let mut is_leaf: Vec<bool> = Vec::new();
    let mut text_buf = String::new();

    // Shared report-level RefDt — captured once when we see it
    // outside any position-set wrapper, then stamped on every
    // subsequent record.
    let mut report_ref_date: Option<NaiveDate> = None;

    let mut current: Option<EmirPositionSetRecord> = None;
    let mut rec_depth: Option<usize> = None;
    let mut records: Vec<EmirPositionSetRecord> = Vec::new();
    // Per-kind 1-based index — synthesises stable record_ids.
    let mut idx_pos_set: u32 = 0;
    let mut idx_ccy_pos_set: u32 = 0;
    let mut idx_coll_pos_set: u32 = 0;
    let mut idx_ccy_coll_pos_set: u32 = 0;

    loop {
        match reader.read_resolved_event_into(&mut buf)? {
            (_, Event::Start(e)) => {
                let local = local_name(&e);
                push_element(&mut pile, &mut is_leaf, local.clone());
                text_buf.clear();

                // Detect a position-set record start: wrapper
                // element name is one of the 4 known kinds and
                // we're inside the Rpt subtree.
                if current.is_none() && pile.iter().any(|s| s == "Rpt") {
                    if let Some(kind) = wrapper_kind(&local) {
                        let index = match kind {
                            "PosSet" => {
                                idx_pos_set += 1;
                                idx_pos_set
                            }
                            "CcyPosSet" => {
                                idx_ccy_pos_set += 1;
                                idx_ccy_pos_set
                            }
                            "CollPosSet" => {
                                idx_coll_pos_set += 1;
                                idx_coll_pos_set
                            }
                            "CcyCollPosSet" => {
                                idx_ccy_coll_pos_set += 1;
                                idx_ccy_coll_pos_set
                            }
                            _ => unreachable!(),
                        };
                        current = Some(EmirPositionSetRecord {
                            source_file: Some(source_label.clone()),
                            record_id: Some(format!("{source_label}#{kind}-{index}")),
                            regime: Regime::Emir,
                            reference_date: report_ref_date,
                            position_set_kind: Some(kind.to_owned()),
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
                    // Capture report-level RefDt the first time
                    // we see it outside a position-set wrapper.
                    if current.is_none()
                        && report_ref_date.is_none()
                        && pile.last().map(String::as_str) == Some("RefDt")
                        && pile.iter().any(|s| s == "Rpt")
                    {
                        if let Ok(d) = NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
                            report_ref_date = Some(d);
                        }
                    }
                    // In-record leaf commit.
                    if let (Some(rec), Some(rdepth)) = (current.as_mut(), rec_depth) {
                        if pile.len() > rdepth {
                            commit_leaf(rec, &pile[rdepth..], trimmed);
                        }
                    }
                }

                // Close the position-set record.
                if let Some(rdepth) = rec_depth {
                    if pile.len() == rdepth {
                        if let Some(mut rec) = current.take() {
                            // Backfill RefDt for early records if
                            // we only saw RefDt later (defensive —
                            // XSD orders RefDt before PosSet[], but
                            // be safe).
                            if rec.reference_date.is_none() {
                                rec.reference_date = report_ref_date;
                            }
                            records.push(rec);
                        }
                        rec_depth = None;
                    }
                }

                pop_element(&mut pile, &mut is_leaf);
                text_buf.clear();
            }
            (_, Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(EmirPositionSetXmlReadOutcome {
        records,
        issues: Vec::new(),
    })
}

fn tail(rel: &[String], suffix: &[&str]) -> bool {
    rel.len() >= suffix.len()
        && rel[rel.len() - suffix.len()..]
            .iter()
            .zip(suffix)
            .all(|(a, b)| a == *b)
}

fn has(rel: &[String], seg: &str) -> bool {
    rel.iter().any(|s| s == seg)
}

fn commit_leaf(rec: &mut EmirPositionSetRecord, rel: &[String], value: &str) {
    if value.is_empty() {
        return;
    }
    let record_id = rec.record_id.clone().unwrap_or_default();

    // Counterparties — LEI tail under RptgCtrPty / OthrCtrPty
    // anywhere in the Dmnsns subtree.
    if tail(rel, &["LEI"]) && has(rel, "RptgCtrPty") {
        rec.reporting_counterparty = Some(value.to_owned());
        return;
    }
    if tail(rel, &["LEI"]) && has(rel, "OthrCtrPty") {
        rec.other_counterparty = Some(value.to_owned());
        return;
    }
    // Dmnsns leaves.
    if tail(rel, &["AsstClss"]) && has(rel, "Dmnsns") {
        rec.asset_class = Some(value.to_owned());
        return;
    }
    if tail(rel, &["CtrctTp"]) && has(rel, "Dmnsns") {
        rec.contract_type = Some(value.to_owned());
        return;
    }
    if tail(rel, &["ValCcy"]) && has(rel, "Dmnsns") {
        rec.value_currency = Some(value.to_owned());
        return;
    }
    if tail(rel, &["ISIN"]) && has(rel, "UndrlygInstrm") {
        rec.underlying_id = Some(value.to_owned());
        return;
    }
    // Metrics — first-wins on each of the 3 headline numbers.
    // PosSet / CcyPosSet: Mtrcs/Ttl/Buyr/Ntnl/Amt → notional.
    if rec.notional.is_none()
        && tail(rel, &["Ntnl", "Amt"])
        && has(rel, "Mtrcs")
        && has(rel, "Buyr")
    {
        set_decimal(&mut rec.notional, value, "notional", &record_id);
        return;
    }
    // PosSet / CcyPosSet: Mtrcs/Ttl/Buyr/PostvVal → MtM proxy.
    if rec.mark_to_market_value.is_none()
        && tail(rel, &["PostvVal"])
        && has(rel, "Mtrcs")
        && has(rel, "Buyr")
    {
        set_decimal(
            &mut rec.mark_to_market_value,
            value,
            "mark_to_market_value",
            &record_id,
        );
        return;
    }
    // CollPosSet / CcyCollPosSet: Mtrcs/Ttl/PstdMrgnOrColl/.../Amt
    // → collateral_value (first-wins across IM/VM/Xcss sub-tags).
    if rec.collateral_value.is_none()
        && tail(rel, &["Amt"])
        && has(rel, "Mtrcs")
        && has(rel, "PstdMrgnOrColl")
    {
        set_decimal(
            &mut rec.collateral_value,
            value,
            "collateral_value",
            &record_id,
        );
        return;
    }

    rec.raw_fields.insert(rel.join("/"), value.to_owned());
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

fn set_decimal(dst: &mut Option<Decimal>, s: &str, field: &str, record: &str) {
    if s.is_empty() {
        return;
    }
    match Decimal::from_str(s) {
        Ok(d) => *dst = Some(d),
        Err(e) => warn!(record = %record, field, value = s, error = %e, "could not parse decimal"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_tmp(name: &str, content: &[u8]) -> std::path::PathBuf {
        let p =
            std::env::temp_dir().join(format!("opendqi-emir-pos-{}-{name}", std::process::id()));
        std::fs::write(&p, content).unwrap();
        p
    }

    const TWO_POSSETS_PLUS_COLL: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.090.001.02">
  <DerivsTradPosSetRpt>
    <AggtdPos>
      <Rpt>
        <RefDt>2026-05-21</RefDt>
        <PosSet>
          <Dmnsns>
            <CtrPtyId>
              <RptgCtrPty><Id><LEI>RPTG-LEI-1</LEI></Id></RptgCtrPty>
              <OthrCtrPty><Id><LEI>OTHR-LEI-1</LEI></Id></OthrCtrPty>
            </CtrPtyId>
            <ValCcy>EUR</ValCcy>
            <AsstClss>CRDT</AsstClss>
            <CtrctTp>SWAP</CtrctTp>
            <UndrlygInstrm><Sngl><ISIN>DE000A1B2C34</ISIN></Sngl></UndrlygInstrm>
          </Dmnsns>
          <Mtrcs>
            <Ttl>
              <Buyr>
                <NbOfTrds>50</NbOfTrds>
                <PostvVal Ccy="EUR">125000.50</PostvVal>
                <NegVal Ccy="EUR">0.00</NegVal>
                <Ntnl><Amt Ccy="EUR">12500000.00</Amt></Ntnl>
              </Buyr>
              <Sellr><NbOfTrds>0</NbOfTrds><PostvVal Ccy="EUR">0</PostvVal><NegVal Ccy="EUR">0</NegVal><Ntnl><Amt Ccy="EUR">0</Amt></Ntnl></Sellr>
            </Ttl>
            <Clean>
              <Buyr><NbOfTrds>50</NbOfTrds><PostvVal Ccy="EUR">125000.50</PostvVal><NegVal Ccy="EUR">0</NegVal><Ntnl><Amt Ccy="EUR">12500000.00</Amt></Ntnl></Buyr>
              <Sellr><NbOfTrds>0</NbOfTrds><PostvVal Ccy="EUR">0</PostvVal><NegVal Ccy="EUR">0</NegVal><Ntnl><Amt Ccy="EUR">0</Amt></Ntnl></Sellr>
            </Clean>
          </Mtrcs>
        </PosSet>
        <PosSet>
          <Dmnsns>
            <CtrPtyId>
              <RptgCtrPty><Id><LEI>RPTG-LEI-1</LEI></Id></RptgCtrPty>
              <OthrCtrPty><Id><LEI>OTHR-LEI-2</LEI></Id></OthrCtrPty>
            </CtrPtyId>
            <ValCcy>USD</ValCcy>
            <AsstClss>INTR</AsstClss>
            <CtrctTp>FUTR</CtrctTp>
          </Dmnsns>
          <Mtrcs>
            <Ttl>
              <Buyr><NbOfTrds>10</NbOfTrds><PostvVal Ccy="USD">5000</PostvVal><NegVal Ccy="USD">0</NegVal><Ntnl><Amt Ccy="USD">100000</Amt></Ntnl></Buyr>
              <Sellr><NbOfTrds>0</NbOfTrds><PostvVal Ccy="USD">0</PostvVal><NegVal Ccy="USD">0</NegVal><Ntnl><Amt Ccy="USD">0</Amt></Ntnl></Sellr>
            </Ttl>
            <Clean>
              <Buyr><NbOfTrds>10</NbOfTrds><PostvVal Ccy="USD">5000</PostvVal><NegVal Ccy="USD">0</NegVal><Ntnl><Amt Ccy="USD">100000</Amt></Ntnl></Buyr>
              <Sellr><NbOfTrds>0</NbOfTrds><PostvVal Ccy="USD">0</PostvVal><NegVal Ccy="USD">0</NegVal><Ntnl><Amt Ccy="USD">0</Amt></Ntnl></Sellr>
            </Clean>
          </Mtrcs>
        </PosSet>
        <CollPosSet>
          <Dmnsns>
            <CtrPtyId>
              <RptgCtrPty><Id><LEI>RPTG-LEI-1</LEI></Id></RptgCtrPty>
              <OthrCtrPty><Id><LEI>OTHR-LEI-1</LEI></Id></OthrCtrPty>
            </CtrPtyId>
            <ValCcy>EUR</ValCcy>
            <AsstClss>CRDT</AsstClss>
            <CtrctTp>SWAP</CtrctTp>
          </Dmnsns>
          <Mtrcs>
            <Ttl>
              <NbOfRpts>50</NbOfRpts>
              <PstdMrgnOrColl><InitlMrgnPstdPreHrcut><Amt Ccy="EUR">110000.25</Amt></InitlMrgnPstdPreHrcut></PstdMrgnOrColl>
            </Ttl>
            <Clean>
              <NbOfRpts>50</NbOfRpts>
            </Clean>
          </Mtrcs>
        </CollPosSet>
      </Rpt>
    </AggtdPos>
  </DerivsTradPosSetRpt>
</Document>"#;

    #[test]
    fn parses_two_possets_plus_one_collposset_record_count() {
        let p = write_tmp("3rec.xml", TWO_POSSETS_PLUS_COLL);
        let outcome = read_emir_position_set_xml(&p).expect("parse");
        assert!(outcome.issues.is_empty(), "no format issues");
        assert_eq!(outcome.records.len(), 3);
        let kinds: Vec<&str> = outcome
            .records
            .iter()
            .map(|r| r.position_set_kind.as_deref().unwrap())
            .collect();
        assert_eq!(kinds, vec!["PosSet", "PosSet", "CollPosSet"]);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn first_posset_extracts_dmnsns_and_metrics_correctly() {
        let p = write_tmp("dmnsns.xml", TWO_POSSETS_PLUS_COLL);
        let outcome = read_emir_position_set_xml(&p).expect("parse");
        let r = &outcome.records[0];
        assert_eq!(r.regime, Regime::Emir);
        assert_eq!(r.reference_date, NaiveDate::from_ymd_opt(2026, 5, 21));
        assert_eq!(r.position_set_kind.as_deref(), Some("PosSet"));
        assert_eq!(r.reporting_counterparty.as_deref(), Some("RPTG-LEI-1"));
        assert_eq!(r.other_counterparty.as_deref(), Some("OTHR-LEI-1"));
        assert_eq!(r.asset_class.as_deref(), Some("CRDT"));
        assert_eq!(r.contract_type.as_deref(), Some("SWAP"));
        assert_eq!(r.value_currency.as_deref(), Some("EUR"));
        assert_eq!(r.underlying_id.as_deref(), Some("DE000A1B2C34"));
        assert_eq!(r.notional, Some(Decimal::from_str("12500000.00").unwrap()));
        assert_eq!(
            r.mark_to_market_value,
            Some(Decimal::from_str("125000.50").unwrap())
        );
        assert!(r.collateral_value.is_none());
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn collposset_captures_collateral_value_not_notional() {
        let p = write_tmp("coll.xml", TWO_POSSETS_PLUS_COLL);
        let outcome = read_emir_position_set_xml(&p).expect("parse");
        let r = &outcome.records[2];
        assert_eq!(r.position_set_kind.as_deref(), Some("CollPosSet"));
        assert!(r.notional.is_none());
        assert!(r.mark_to_market_value.is_none());
        assert_eq!(
            r.collateral_value,
            Some(Decimal::from_str("110000.25").unwrap())
        );
        let _ = std::fs::remove_file(&p);
    }

    const NOTX: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.090.001.02">
  <DerivsTradPosSetRpt>
    <AggtdPos>
      <DataSetActn>NOTX</DataSetActn>
    </AggtdPos>
  </DerivsTradPosSetRpt>
</Document>"#;

    #[test]
    fn no_records_with_dataset_actn() {
        let p = write_tmp("notx.xml", NOTX);
        let outcome = read_emir_position_set_xml(&p).expect("parse");
        assert!(outcome.records.is_empty());
        assert!(outcome.issues.is_empty());
        let _ = std::fs::remove_file(&p);
    }

    const WRONG_NS: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.999.001.02"><X/></Document>"#;

    #[test]
    fn wrong_namespace_emits_warning_no_records() {
        let p = write_tmp("wrong_ns.xml", WRONG_NS);
        let outcome = read_emir_position_set_xml(&p).expect("parse");
        assert!(outcome.records.is_empty());
        assert_eq!(outcome.issues.len(), 1);
        assert_eq!(
            outcome.issues[0].check_id,
            "EMIR.FMT.XML_UNSUPPORTED_NAMESPACE"
        );
        let _ = std::fs::remove_file(&p);
    }
}
