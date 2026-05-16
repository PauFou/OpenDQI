//! SFTR reconciliation ingestion — the **real** ISO 20022
//! `auth.080.001.02` *Securities Financing Reporting Reconciliation
//! Status Advice* (`opendqi sftr reconcile`).
//!
//! EMIR has **no** counterparty pairing/reconciliation message (real
//! `auth.106` is a data-quality *warnings* report — see
//! `crates/opendqi-xml/src/emir_warnings.rs` and
//! `docs/auth-messages/emir-auth106.md`). The earlier synthetic
//! `auth.106`/`auth.083` "pairing" shape and the `opendqi emir
//! reconcile` command were removed in Milestone 0.6; real
//! `auth.083` is a *Missing Collateral Request* and is not modelled
//! yet.
//!
//! ## Legal note
//!
//! The SWIFT-licensed `auth.080` XSD is **not** redistributed with
//! OpenDQI; only the schema *shape* is encoded (documented
//! derive-subset — see `docs/auth-messages/sftr-auth080.md`).

use std::path::Path;

use opendqi_core::{DqDimension, DqIssue, ReconciliationRecord, Regime, Severity};
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::NsReader;

use crate::wellformed::check_wellformedness;

/// Outcome of reading one TR reconciliation XML file.
#[derive(Debug, Default)]
pub struct ReconciliationXmlReadOutcome {
    /// Records extracted from the file.
    pub records: Vec<ReconciliationRecord>,
    /// File-level data-quality issues (format / namespace).
    pub issues: Vec<DqIssue>,
}

/// Real ESMA SFTR Reconciliation Status Advice
/// (`auth.080.001.02_ESMAUG_SFTREC_1.1.0`).
const ISO20022_AUTH_080_NS: &[u8] = b"urn:iso:std:iso:20022:tech:xsd:auth.080.001.02";

/// Real auth.080 per-transaction record element.
const RCNCLTN_RPT_BLOCK: &str = "RcncltnRpt";

/// Read an SFTR reconciliation file — the **real** `auth.080.001.02`
/// Reconciliation Status Advice. (EMIR has no reconciliation message;
/// real `auth.083` is a Missing Collateral Request, not modelled.)
pub fn read_sftr_reconciliation_xml(path: &Path) -> anyhow::Result<ReconciliationXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();
    if let Err(err) = check_wellformedness(path) {
        return Ok(ReconciliationXmlReadOutcome {
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
        Some(ns) if ns == ISO20022_AUTH_080_NS => parse_real_auth080(path),
        other => {
            let actual = other
                .as_deref()
                .map(|n| String::from_utf8_lossy(n).into_owned())
                .unwrap_or_else(|| "(none)".into());
            Ok(ReconciliationXmlReadOutcome {
                records: vec![],
                issues: vec![fmt_issue(
                    "SFTR.FMT.XML_UNSUPPORTED_NAMESPACE",
                    Severity::Warning,
                    format!(
                        "Root namespace is '{actual}', expected \
                         'urn:iso:std:iso:20022:tech:xsd:auth.080.001.02' \
                         (SFTR Reconciliation Status Advice)."
                    ),
                    source_label,
                )],
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

/// True when `rel` ends with `suffix`.
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

/// Parse the **real** SFTR `auth.080.001.02` Reconciliation Status
/// Advice into the canonical `ReconciliationRecord` (documented
/// derive-subset; the SWIFT-licensed XSD is not redistributed). See
/// `docs/auth-messages/sftr-auth080.md`.
///
/// ```text
/// Document/SctiesFincgRptgRcncltnStsAdvc/RcncltnData
///   ├─ DataSetActn = "NOTX"            (no-activity)
///   └─ Rpt/RcncltnRpt (record):
///        TxId/{UnqTradIdr, RptgCtrPty/LEI, OthrCtrPty/Lgl/LEI}
///        RcncltnSts: NoRcncltnReqrd | RptgData/{Mtchd | NotMtchd/
///          MtchgCrit/{CtrPty,Ln,Coll}MtchgCrit/<criterion> ...}
/// ```
/// Mtchd ⇒ PAIRED/RECONCILED; NotMtchd ⇒ PAIRED/UNRECONCILED +
/// mismatched_fields = the criterion element names; NoRcncltnReqrd ⇒
/// no status assertion. `UNPAIRED` is only a `PairgRcncltnSts` summary
/// count (not per-record) → `SFTR.REC.UNPAIRED_TRADE` is unreachable.
fn parse_real_auth080(path: &Path) -> anyhow::Result<ReconciliationXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();
    let mut reader = NsReader::from_file(path)?;
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut pile: Vec<String> = Vec::new();
    let mut is_leaf: Vec<bool> = Vec::new();
    let mut text_buf = String::new();

    let mut current: Option<ReconciliationRecord> = None;
    let mut rec_depth: Option<usize> = None;
    let mut records: Vec<ReconciliationRecord> = Vec::new();
    let mut rec_index: u32 = 0;
    let mut saw_dataset_actn = false;

    const CRIT_PARENTS: [&str; 3] = ["CtrPtyMtchgCrit", "LnMtchgCrit", "CollMtchgCrit"];

    loop {
        match reader.read_resolved_event_into(&mut buf)? {
            (_, Event::Start(e)) => {
                let local = local_name(&e);
                push_element(&mut pile, &mut is_leaf, local);
                text_buf.clear();

                if current.is_none()
                    && pile.last().map(String::as_str) == Some(RCNCLTN_RPT_BLOCK)
                    && pile.iter().any(|s| s == "RcncltnData")
                {
                    rec_index += 1;
                    current = Some(ReconciliationRecord {
                        source_file: Some(source_label.clone()),
                        record_id: Some(format!("{source_label}#rcn-{rec_index}")),
                        regime: Regime::Sftr,
                        ..Default::default()
                    });
                    rec_depth = Some(pile.len());
                } else if let Some(rec) = current.as_mut() {
                    match pile.last().map(String::as_str) {
                        Some("Mtchd") => {
                            rec.pairing_status = Some("PAIRED".into());
                            rec.reconciliation_status = Some("RECONCILED".into());
                        }
                        Some("NotMtchd") => {
                            rec.pairing_status = Some("PAIRED".into());
                            rec.reconciliation_status = Some("UNRECONCILED".into());
                        }
                        Some(name) => {
                            // A mismatched criterion: direct child of a
                            // *MtchgCrit under NotMtchd.
                            let n = pile.len();
                            if n >= 2
                                && CRIT_PARENTS.contains(&pile[n - 2].as_str())
                                && pile.iter().any(|s| s == "NotMtchd")
                                && !rec.mismatched_fields.iter().any(|f| f == name)
                            {
                                rec.mismatched_fields.push(name.to_owned());
                            }
                        }
                        None => {}
                    }
                }
            }
            (_, Event::Empty(e)) => {
                let local = local_name(&e);
                push_element(&mut pile, &mut is_leaf, local.clone());
                // Empty `<Mtchd/>` still flips the status.
                if let Some(rec) = current.as_mut() {
                    if local == "Mtchd" {
                        rec.pairing_status = Some("PAIRED".into());
                        rec.reconciliation_status = Some("RECONCILED".into());
                    }
                }
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
                    let v = text_buf.trim();
                    if current.is_none()
                        && pile.last().map(String::as_str) == Some("DataSetActn")
                        && pile.iter().any(|s| s == "RcncltnData")
                    {
                        saw_dataset_actn = true;
                    }
                    if let (Some(rec), Some(rdepth)) = (current.as_mut(), rec_depth) {
                        if pile.len() > rdepth && !v.is_empty() {
                            let rel = &pile[rdepth..];
                            if tail(rel, &["UnqTradIdr"])
                                && has(rel, "TxId")
                                && !has(rel, "MtchgCrit")
                            {
                                rec.uti = Some(v.to_owned());
                            } else if tail(rel, &["LEI"]) && has(rel, "TxId") {
                                if has(rel, "RptgCtrPty") {
                                    rec.reporting_counterparty = Some(v.to_owned());
                                } else if has(rel, "OthrCtrPty") {
                                    rec.other_counterparty = Some(v.to_owned());
                                }
                            }
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
            }
            (_, Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }

    let mut issues = Vec::new();
    if records.is_empty() && saw_dataset_actn {
        issues.push(fmt_issue(
            "SFTR.FMT.RCNCLN_NO_RECORDS",
            Severity::Info,
            "Reconciliation Status Advice carries RcncltnData/DataSetActn \
             (no-activity report); zero reconciliation records to evaluate."
                .to_string(),
            source_label,
        ));
    }

    Ok(ReconciliationXmlReadOutcome { records, issues })
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

#[cfg(test)]
mod tests {
    use super::*;

    fn write_tmp(name: &str, content: &[u8]) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("opendqi-recon-{}-{name}", std::process::id()));
        std::fs::write(&p, content).unwrap();
        p
    }

    const REAL_AUTH080: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.080.001.02">
  <SctiesFincgRptgRcncltnStsAdvc>
    <RcncltnData>
      <Rpt>
        <PairgRcncltnSts><DtldNbOfRpts>3</DtldNbOfRpts><DtldSts>RECO</DtldSts></PairgRcncltnSts>
        <RcncltnRpt>
          <TechRcrdId>REC-1</TechRcrdId>
          <TxId>
            <RptgCtrPty><LEI>RPTGLEI0000000000001</LEI></RptgCtrPty>
            <OthrCtrPty><Lgl><LEI>OTHRLEI0000000000002</LEI></Lgl></OthrCtrPty>
            <UnqTradIdr>SFTR-RECON-MTCHD-1</UnqTradIdr>
          </TxId>
          <Modfd>false</Modfd>
          <RcncltnSts><RptgData><Mtchd/></RptgData></RcncltnSts>
        </RcncltnRpt>
        <RcncltnRpt>
          <TxId>
            <RptgCtrPty><LEI>RPTGLEI0000000000001</LEI></RptgCtrPty>
            <OthrCtrPty><Lgl><LEI>OTHRLEI0000000000002</LEI></Lgl></OthrCtrPty>
            <UnqTradIdr>SFTR-RECON-NOTMTCHD-2</UnqTradIdr>
          </TxId>
          <Modfd>true</Modfd>
          <RcncltnSts><RptgData><NotMtchd>
            <CtrPty1><LEI>RPTGLEI0000000000001</LEI></CtrPty1>
            <CtrPty2><LEI>OTHRLEI0000000000002</LEI></CtrPty2>
            <MtchgCrit>
              <LnMtchgCrit>
                <TermntnDt><Val1>2026-01-01</Val1><Val2>2026-02-01</Val2></TermntnDt>
                <MtrtyDt><Val1>2030-01-01</Val1><Val2>2030-06-01</Val2></MtrtyDt>
              </LnMtchgCrit>
              <CollMtchgCrit>
                <CollValDt><Val1>2026-05-13</Val1><Val2>2026-05-14</Val2></CollValDt>
              </CollMtchgCrit>
            </MtchgCrit>
          </NotMtchd></RptgData></RcncltnSts>
        </RcncltnRpt>
        <RcncltnRpt>
          <TxId><UnqTradIdr>SFTR-RECON-NOREQ-3</UnqTradIdr></TxId>
          <Modfd>false</Modfd>
          <RcncltnSts><NoRcncltnReqrd>NORE</NoRcncltnReqrd></RcncltnSts>
        </RcncltnRpt>
      </Rpt>
    </RcncltnData>
  </SctiesFincgRptgRcncltnStsAdvc>
</Document>"#;

    #[test]
    fn parses_real_auth080_reconciliation() {
        let p = write_tmp("a080.xml", REAL_AUTH080);
        let out = read_sftr_reconciliation_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert!(out.issues.is_empty());
        assert_eq!(out.records.len(), 3);

        let r0 = &out.records[0];
        assert_eq!(r0.regime, Regime::Sftr);
        assert_eq!(r0.uti.as_deref(), Some("SFTR-RECON-MTCHD-1"));
        assert_eq!(
            r0.reporting_counterparty.as_deref(),
            Some("RPTGLEI0000000000001")
        );
        assert_eq!(
            r0.other_counterparty.as_deref(),
            Some("OTHRLEI0000000000002")
        );
        assert_eq!(r0.pairing_status.as_deref(), Some("PAIRED"));
        assert_eq!(r0.reconciliation_status.as_deref(), Some("RECONCILED"));
        assert!(r0.mismatched_fields.is_empty());

        let r1 = &out.records[1];
        assert_eq!(r1.uti.as_deref(), Some("SFTR-RECON-NOTMTCHD-2"));
        assert_eq!(r1.reconciliation_status.as_deref(), Some("UNRECONCILED"));
        assert_eq!(
            r1.mismatched_fields,
            vec!["TermntnDt", "MtrtyDt", "CollValDt"]
        );

        let r2 = &out.records[2];
        assert_eq!(r2.uti.as_deref(), Some("SFTR-RECON-NOREQ-3"));
        assert!(r2.pairing_status.is_none() && r2.reconciliation_status.is_none());
    }

    #[test]
    fn auth080_no_activity_yields_info() {
        let body = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.080.001.02">
  <SctiesFincgRptgRcncltnStsAdvc>
    <RcncltnData><DataSetActn>NOTX</DataSetActn></RcncltnData>
  </SctiesFincgRptgRcncltnStsAdvc>
</Document>"#;
        let p = write_tmp("a080-empty.xml", body);
        let out = read_sftr_reconciliation_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert!(out.records.is_empty());
        assert_eq!(out.issues.len(), 1);
        assert_eq!(out.issues[0].check_id, "SFTR.FMT.RCNCLN_NO_RECORDS");
    }

    #[test]
    fn sftr_unsupported_namespace_yields_warning() {
        let body = br#"<?xml version="1.0"?><Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.030.001.03"/>"#;
        let p = write_tmp("a080-bad.xml", body);
        let out = read_sftr_reconciliation_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert!(out.records.is_empty());
        assert_eq!(out.issues[0].check_id, "SFTR.FMT.XML_UNSUPPORTED_NAMESPACE");
    }
}
