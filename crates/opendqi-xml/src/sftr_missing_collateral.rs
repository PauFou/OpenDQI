//! SFTR Missing Collateral Request ingestion — ISO 20022 `auth.083`.
//!
//! Element paths are aligned with the real ESMA SFTR usage guideline
//! `auth.083.001.02_ESMAUG_1.0.0`
//! (`SecuritiesFinancingReportingMissingCollateralRequestV02`). The
//! SWIFT-licensed XSD is **not** redistributed; only the schema *shape*
//! is encoded. Coverage is a documented **subset** projected onto the
//! scalar `MissingCollateralRecord` — see
//! `docs/auth-messages/sftr-auth083.md`.
//!
//! Real envelope:
//! ```text
//! Document
//! └─ SctiesFincgRptgMssngCollReq  (…MissingCollateralRequestV02)
//!    └─ TxId (1..n)  (TradeTransactionIdentification18)   ← RECORD
//!       ├─ RptgCtrPty/LEI                  [1..1]
//!       ├─ OthrCtrPty: Lgl/LEI | Ntrl/Id/Id [1..1]
//!       ├─ UnqTradIdr  (Max52Text)         [0..1]  (the UTI)
//!       └─ MstrAgrmt   [0..1]: Tp/Tp(code), Vrsn, OthrMstrAgrmtDtls
//! ```
//! The TR is **requesting** the firm to provide missing collateral for
//! each listed SFT. `TxId` is mandatory (≥1) so a valid instance always
//! has records — there is no `DataSetActn`/NOTX no-activity branch.

use std::path::Path;

use opendqi_core::{DqDimension, DqIssue, MissingCollateralRecord, Regime, Severity};
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::NsReader;

use crate::wellformed::check_wellformedness;

/// Outcome of reading one auth.083 XML file.
#[derive(Debug, Default)]
pub struct MissingCollateralXmlReadOutcome {
    /// Records extracted from the file (one per `TxId`).
    pub records: Vec<MissingCollateralRecord>,
    /// File-level data-quality / parse issues (format / namespace).
    pub issues: Vec<DqIssue>,
}

const ISO20022_AUTH_083_NS: &[u8] = b"urn:iso:std:iso:20022:tech:xsd:auth.083.001.02";

/// Read an SFTR `auth.083` Missing Collateral Request file.
pub fn read_sftr_missing_collateral_xml(
    path: &Path,
) -> anyhow::Result<MissingCollateralXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();

    if let Err(err) = check_wellformedness(path) {
        return Ok(MissingCollateralXmlReadOutcome {
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
        Some(ns) if ns == ISO20022_AUTH_083_NS => parse(path),
        other => {
            let actual = other
                .as_deref()
                .map(|n| String::from_utf8_lossy(n).into_owned())
                .unwrap_or_else(|| "(none)".into());
            Ok(MissingCollateralXmlReadOutcome {
                records: vec![],
                issues: vec![fmt_issue(
                    "SFTR.FMT.XML_UNSUPPORTED_NAMESPACE",
                    Severity::Warning,
                    format!(
                        "Root namespace is '{actual}', expected 'urn:iso:std:iso:20022:tech:xsd:auth.083.001.02'."
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

/// True when `seg` appears anywhere in `pile`.
fn has(pile: &[String], seg: &str) -> bool {
    pile.iter().any(|s| s == seg)
}

fn parse(path: &Path) -> anyhow::Result<MissingCollateralXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();
    let mut reader = NsReader::from_file(path)?;
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut pile: Vec<String> = Vec::new();
    let mut is_leaf: Vec<bool> = Vec::new();
    let mut text_buf = String::new();

    let mut tx_depth: Option<usize> = None;
    let mut cur: Option<MissingCollateralRecord> = None;
    let mut records: Vec<MissingCollateralRecord> = Vec::new();
    let mut seq: usize = 0;

    loop {
        match reader.read_resolved_event_into(&mut buf)? {
            (_, Event::Start(e)) => {
                let local = local_name(&e);
                push_element(&mut pile, &mut is_leaf, local);
                text_buf.clear();

                if pile.last().map(String::as_str) == Some("TxId")
                    && has(&pile, "SctiesFincgRptgMssngCollReq")
                    && tx_depth.is_none()
                {
                    tx_depth = Some(pile.len());
                    seq += 1;
                    cur = Some(MissingCollateralRecord {
                        source_file: Some(source_label.clone()),
                        record_id: Some(format!("{source_label}#mcr-{seq}")),
                        regime: Regime::Sftr,
                        ..Default::default()
                    });
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
                if leaf_now && tx_depth.is_some() {
                    let v = text_buf.trim();
                    if !v.is_empty() {
                        if let Some(rec) = cur.as_mut() {
                            let leaf = pile.last().map(String::as_str).unwrap_or("");
                            if leaf == "LEI"
                                && has(&pile, "RptgCtrPty")
                                && !has(&pile, "OthrCtrPty")
                            {
                                rec.reporting_counterparty = Some(v.to_owned());
                            } else if has(&pile, "OthrCtrPty")
                                && rec.other_counterparty.is_none()
                                && (leaf == "LEI" || leaf == "Id")
                            {
                                // OthrCtrPty/Lgl/LEI or OthrCtrPty/Ntrl/Id/Id.
                                rec.other_counterparty = Some(v.to_owned());
                            } else if leaf == "UnqTradIdr" {
                                rec.uti = Some(v.to_owned());
                            } else if leaf == "Tp" && has(&pile, "MstrAgrmt") {
                                rec.master_agreement_type = Some(v.to_owned());
                            } else if leaf == "Vrsn" && has(&pile, "MstrAgrmt") {
                                rec.master_agreement_version = Some(v.to_owned());
                            } else if leaf == "OthrMstrAgrmtDtls" && has(&pile, "MstrAgrmt") {
                                // Free-text master-agreement note: no
                                // typed field — preserved verbatim.
                                rec.raw_fields
                                    .insert("MstrAgrmt/OthrMstrAgrmtDtls".to_owned(), v.to_owned());
                            }
                        }
                    }
                }

                if let Some(td) = tx_depth {
                    if pile.len() == td {
                        if let Some(rec) = cur.take() {
                            records.push(rec);
                        }
                        tx_depth = None;
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

    Ok(MissingCollateralXmlReadOutcome {
        records,
        issues: Vec::new(),
    })
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
        let p = std::env::temp_dir().join(format!("opendqi-mcr-{}-{name}", std::process::id()));
        std::fs::write(&p, content).unwrap();
        p
    }

    const REAL_ENVELOPE: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.083.001.02">
  <SctiesFincgRptgMssngCollReq>
    <TxId>
      <RptgCtrPty><LEI>ABCDEFGHIJKLMNOPQR99</LEI></RptgCtrPty>
      <OthrCtrPty><Lgl><LEI>ZYXWVUTSRQPONMLKJI88</LEI></Lgl></OthrCtrPty>
      <UnqTradIdr>UTI-MCR-0001</UnqTradIdr>
      <MstrAgrmt><Tp><Tp>GMRA</Tp></Tp><Vrsn>2011</Vrsn><OthrMstrAgrmtDtls>BILATERAL ANNEX 2020</OthrMstrAgrmtDtls></MstrAgrmt>
    </TxId>
    <TxId>
      <RptgCtrPty><LEI>ABCDEFGHIJKLMNOPQR99</LEI></RptgCtrPty>
      <OthrCtrPty><Ntrl><Id><Id>NATURAL-PERSON-1</Id></Id></Ntrl></OthrCtrPty>
    </TxId>
  </SctiesFincgRptgMssngCollReq>
</Document>"#;

    #[test]
    fn parses_records_per_txid() {
        let p = write_tmp("real.xml", REAL_ENVELOPE);
        let out = read_sftr_missing_collateral_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert!(out.issues.is_empty());
        assert_eq!(out.records.len(), 2);
        let r0 = &out.records[0];
        assert_eq!(r0.regime, Regime::Sftr);
        assert_eq!(
            r0.reporting_counterparty.as_deref(),
            Some("ABCDEFGHIJKLMNOPQR99")
        );
        assert_eq!(
            r0.other_counterparty.as_deref(),
            Some("ZYXWVUTSRQPONMLKJI88")
        );
        assert_eq!(r0.uti.as_deref(), Some("UTI-MCR-0001"));
        assert_eq!(r0.master_agreement_type.as_deref(), Some("GMRA"));
        assert_eq!(r0.master_agreement_version.as_deref(), Some("2011"));
        assert_eq!(
            r0.raw_fields
                .get("MstrAgrmt/OthrMstrAgrmtDtls")
                .map(String::as_str),
            Some("BILATERAL ANNEX 2020")
        );
        // Second record: natural-person other CP, no UTI, no
        // OthrMstrAgrmtDtls → raw_fields stays empty.
        let r1 = &out.records[1];
        assert_eq!(r1.other_counterparty.as_deref(), Some("NATURAL-PERSON-1"));
        assert!(r1.uti.is_none());
        assert!(r1.raw_fields.is_empty());
    }

    #[test]
    fn unsupported_namespace_yields_warning() {
        let body = br#"<?xml version="1.0"?><Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.080.001.02"/>"#;
        let p = write_tmp("wrong.xml", body);
        let out = read_sftr_missing_collateral_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert!(out.records.is_empty());
        assert_eq!(out.issues[0].check_id, "SFTR.FMT.XML_UNSUPPORTED_NAMESPACE");
    }
}
