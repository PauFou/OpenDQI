//! SFTR Transaction Status Advice (ISO 20022 `auth.084`)
//! ingestion.
//!
//! Element paths are aligned with the real ESMA SFTR usage
//! guideline `auth.084.001.02_ESMAUG_1.0.0`
//! (`SecuritiesFinancingReportingTransactionStatusAdviceV02`).
//! The SWIFT-licensed XSD is **not** redistributed; only the
//! schema *shape* (element names, nesting, cardinalities) is
//! encoded here.
//!
//! Real envelope:
//! ```text
//! Document
//! └─ SctiesFincgRptgTxStsAdvc           (SecuritiesFinancingReportingTransactionStatusAdviceV02)
//!    └─ TxRptStsAndRsn                  (TradeData35Choice__1)
//!       └─ Rpt   (TradeData29__1, exactly 1)
//!          ├─ RptSttstcs (DetailedReportStatistics5)
//!          │  ├─ TtlNbOfRpts                              → total_reports
//!          │  ├─ TtlNbOfRptsAccptd                        → total_reports_accepted
//!          │  ├─ TtlNbOfRptsRjctd                         → total_reports_rejected
//!          │  └─ NbOfRptsRjctdPerErr[]
//!          │     ├─ Dtl/.../Id                            → rejected_reports_per_error key
//!          │     └─ NbOfTxs                               → rejected_reports_per_error value
//!          └─ TxSttstcs                                   → raw_fields (per-tx breakdown)
//! ```
//!
//! Unlike the per-trade SFTR messages, `auth.084` is a
//! statistics shape: one logical record per file carrying
//! aggregate counts. The parser models it that way — one
//! `SftrTrStatusAdviceRecord` per `Rpt` element.

use std::path::Path;
use std::str::FromStr;

use opendqi_core::{DqDimension, DqIssue, Regime, Severity, SftrTrStatusAdviceRecord};
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::NsReader;
use tracing::warn;

use crate::wellformed::check_wellformedness;

/// Outcome of reading one SFTR auth.084 XML file.
#[derive(Debug, Default)]
pub struct SftrTrStatusAdviceXmlReadOutcome {
    /// Records extracted from the file (typically exactly 1).
    pub records: Vec<SftrTrStatusAdviceRecord>,
    /// File-level data-quality / parse issues (format / namespace).
    pub issues: Vec<DqIssue>,
}

const ISO20022_AUTH_084_NS: &[u8] = b"urn:iso:std:iso:20022:tech:xsd:auth.084.001.02";
/// The single per-record element under `TxRptStsAndRsn`.
const RPT_BLOCK: &str = "Rpt";

/// Read an SFTR `auth.084` Transaction Status Advice file.
pub fn read_sftr_tr_status_advice_xml(
    path: &Path,
) -> anyhow::Result<SftrTrStatusAdviceXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();

    if let Err(err) = check_wellformedness(path) {
        return Ok(SftrTrStatusAdviceXmlReadOutcome {
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
        Some(ns) if ns == ISO20022_AUTH_084_NS => parse(path),
        other => {
            let actual = other
                .as_deref()
                .map(|n| String::from_utf8_lossy(n).into_owned())
                .unwrap_or_else(|| "(none)".into());
            Ok(SftrTrStatusAdviceXmlReadOutcome {
                records: vec![],
                issues: vec![fmt_issue(
                    "SFTR.FMT.XML_UNSUPPORTED_NAMESPACE",
                    Severity::Warning,
                    format!(
                        "Root namespace is '{actual}', expected 'urn:iso:std:iso:20022:tech:xsd:auth.084.001.02'."
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

fn parse(path: &Path) -> anyhow::Result<SftrTrStatusAdviceXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();
    let mut reader = NsReader::from_file(path)?;
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut pile: Vec<String> = Vec::new();
    let mut is_leaf: Vec<bool> = Vec::new();
    let mut text_buf = String::new();

    let mut current: Option<SftrTrStatusAdviceRecord> = None;
    let mut rec_depth: Option<usize> = None;
    let mut records: Vec<SftrTrStatusAdviceRecord> = Vec::new();
    let mut rec_index: u32 = 0;

    // Per-error-row accumulator. We capture the (key, count)
    // pair across the children of a single
    // `NbOfRptsRjctdPerErr` element, then commit on its close.
    let mut per_err_active: bool = false;
    let mut per_err_key: Option<String> = None;
    let mut per_err_count: Option<u64> = None;

    loop {
        match reader.read_resolved_event_into(&mut buf)? {
            (_, Event::Start(e)) => {
                let local = local_name(&e);
                push_element(&mut pile, &mut is_leaf, local.clone());
                text_buf.clear();

                if current.is_none()
                    && pile.last().map(String::as_str) == Some(RPT_BLOCK)
                    && pile.iter().any(|s| s == "TxRptStsAndRsn")
                {
                    rec_index += 1;
                    current = Some(SftrTrStatusAdviceRecord {
                        source_file: Some(source_label.clone()),
                        record_id: Some(format!("{source_label}#rpt-{rec_index}")),
                        regime: Regime::Sftr,
                        ..Default::default()
                    });
                    rec_depth = Some(pile.len());
                }

                if current.is_some()
                    && pile.last().map(String::as_str) == Some("NbOfRptsRjctdPerErr")
                {
                    per_err_active = true;
                    per_err_key = None;
                    per_err_count = None;
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
                    if let (Some(rec), Some(rdepth)) = (current.as_mut(), rec_depth) {
                        if pile.len() > rdepth {
                            commit_leaf(
                                rec,
                                &pile[rdepth..],
                                trimmed,
                                per_err_active,
                                &mut per_err_key,
                                &mut per_err_count,
                            );
                        }
                    }
                }

                // Close the NbOfRptsRjctdPerErr block: flush
                // the accumulated (key, count) pair.
                if per_err_active && pile.last().map(String::as_str) == Some("NbOfRptsRjctdPerErr")
                {
                    if let (Some(rec), Some(k), Some(c)) =
                        (current.as_mut(), per_err_key.take(), per_err_count.take())
                    {
                        rec.rejected_reports_per_error.insert(k, c);
                    }
                    per_err_active = false;
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

    Ok(SftrTrStatusAdviceXmlReadOutcome {
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

fn commit_leaf(
    rec: &mut SftrTrStatusAdviceRecord,
    rel: &[String],
    value: &str,
    per_err_active: bool,
    per_err_key: &mut Option<String>,
    per_err_count: &mut Option<u64>,
) {
    if value.is_empty() {
        return;
    }
    let record_id = rec.record_id.clone().unwrap_or_default();

    // Top-level statistics under RptSttstcs.
    if tail(rel, &["TtlNbOfRpts"]) && has(rel, "RptSttstcs") && !per_err_active {
        set_u64(&mut rec.total_reports, value, "TtlNbOfRpts", &record_id);
        return;
    }
    if tail(rel, &["TtlNbOfRptsAccptd"]) && has(rel, "RptSttstcs") {
        set_u64(
            &mut rec.total_reports_accepted,
            value,
            "TtlNbOfRptsAccptd",
            &record_id,
        );
        return;
    }
    if tail(rel, &["TtlNbOfRptsRjctd"]) && has(rel, "RptSttstcs") {
        set_u64(
            &mut rec.total_reports_rejected,
            value,
            "TtlNbOfRptsRjctd",
            &record_id,
        );
        return;
    }

    // Inside a NbOfRptsRjctdPerErr block: collect validation
    // rule id + count. The XSD nests these under a Dtl/<choice>/Id
    // subtree, so we accept any `Id` leaf inside the active per-
    // error scope as the key, and `NbOfTxs` as the count.
    if per_err_active {
        if tail(rel, &["Id"]) && per_err_key.is_none() {
            *per_err_key = Some(value.to_owned());
            return;
        }
        if tail(rel, &["NbOfTxs"]) {
            let mut tmp = None;
            set_u64(&mut tmp, value, "NbOfTxs", &record_id);
            if tmp.is_some() {
                *per_err_count = tmp;
            }
            return;
        }
    }

    // Everything else (TxSttstcs subtree, reporting metadata)
    // goes to raw_fields.
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

fn set_u64(dst: &mut Option<u64>, s: &str, field: &str, record: &str) {
    if s.is_empty() {
        return;
    }
    match u64::from_str(s) {
        Ok(v) => *dst = Some(v),
        Err(e) => warn!(record = %record, field, value = s, error = %e, "could not parse u64"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_tmp(name: &str, content: &[u8]) -> std::path::PathBuf {
        let p =
            std::env::temp_dir().join(format!("opendqi-sftr-tsa-{}-{name}", std::process::id()));
        std::fs::write(&p, content).unwrap();
        p
    }

    const FULL_ADVICE: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.084.001.02">
  <SctiesFincgRptgTxStsAdvc>
    <TxRptStsAndRsn>
      <Rpt>
        <RptSttstcs>
          <TtlNbOfRpts>1000</TtlNbOfRpts>
          <TtlNbOfRptsAccptd>985</TtlNbOfRptsAccptd>
          <TtlNbOfRptsRjctd>15</TtlNbOfRptsRjctd>
          <NbOfRptsRjctdPerErr>
            <Dtl><Prtry><Id>VR-001</Id></Prtry></Dtl>
            <NbOfTxs>12</NbOfTxs>
          </NbOfRptsRjctdPerErr>
          <NbOfRptsRjctdPerErr>
            <Dtl><Prtry><Id>VR-002</Id></Prtry></Dtl>
            <NbOfTxs>3</NbOfTxs>
          </NbOfRptsRjctdPerErr>
        </RptSttstcs>
        <TxSttstcs>
          <DataSetActn>NOTX</DataSetActn>
        </TxSttstcs>
      </Rpt>
    </TxRptStsAndRsn>
  </SctiesFincgRptgTxStsAdvc>
</Document>"#;

    #[test]
    fn parses_full_advice_with_per_error_breakdown() {
        let p = write_tmp("full.xml", FULL_ADVICE);
        let outcome = read_sftr_tr_status_advice_xml(&p).expect("parse");
        assert!(outcome.issues.is_empty(), "no format issues");
        assert_eq!(outcome.records.len(), 1);
        let r = &outcome.records[0];
        assert_eq!(r.regime, Regime::Sftr);
        assert_eq!(r.total_reports, Some(1000));
        assert_eq!(r.total_reports_accepted, Some(985));
        assert_eq!(r.total_reports_rejected, Some(15));
        assert_eq!(r.rejected_reports_per_error.len(), 2);
        assert_eq!(r.rejected_reports_per_error.get("VR-001"), Some(&12));
        assert_eq!(r.rejected_reports_per_error.get("VR-002"), Some(&3));
        let _ = std::fs::remove_file(&p);
    }

    const NO_REJECTIONS: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.084.001.02">
  <SctiesFincgRptgTxStsAdvc>
    <TxRptStsAndRsn>
      <Rpt>
        <RptSttstcs>
          <TtlNbOfRpts>500</TtlNbOfRpts>
          <TtlNbOfRptsAccptd>500</TtlNbOfRptsAccptd>
          <TtlNbOfRptsRjctd>0</TtlNbOfRptsRjctd>
        </RptSttstcs>
        <TxSttstcs>
          <DataSetActn>NOTX</DataSetActn>
        </TxSttstcs>
      </Rpt>
    </TxRptStsAndRsn>
  </SctiesFincgRptgTxStsAdvc>
</Document>"#;

    #[test]
    fn parses_clean_advice_with_no_rejections() {
        let p = write_tmp("clean.xml", NO_REJECTIONS);
        let outcome = read_sftr_tr_status_advice_xml(&p).expect("parse");
        assert_eq!(outcome.records.len(), 1);
        let r = &outcome.records[0];
        assert_eq!(r.total_reports, Some(500));
        assert_eq!(r.total_reports_accepted, Some(500));
        assert_eq!(r.total_reports_rejected, Some(0));
        assert!(r.rejected_reports_per_error.is_empty());
        let _ = std::fs::remove_file(&p);
    }

    const WRONG_NS: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.999.001.02"><X/></Document>"#;

    #[test]
    fn wrong_namespace_emits_warning_no_records() {
        let p = write_tmp("wrong_ns.xml", WRONG_NS);
        let outcome = read_sftr_tr_status_advice_xml(&p).expect("parse");
        assert!(outcome.records.is_empty());
        assert_eq!(outcome.issues.len(), 1);
        assert_eq!(
            outcome.issues[0].check_id,
            "SFTR.FMT.XML_UNSUPPORTED_NAMESPACE"
        );
        let _ = std::fs::remove_file(&p);
    }
}
