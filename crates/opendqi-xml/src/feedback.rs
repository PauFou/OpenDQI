//! TR feedback ingestion — ISO 20022 `auth.092` (EMIR) and `auth.080`
//! (SFTR). One shared adapter, regime-dispatched by namespace.
//!
//! ## EMIR `auth.092` — schema-aligned subset
//!
//! The EMIR path parses the **real** ESMA usage guideline
//! `auth.092.001.04_ESMAUG_DATREJ_1.0.0`
//! (`DerivativesTradeRejectionStatisticalReportV04`) — a rejection
//! *statistics* report, not a per-UTI feedback feed. The SWIFT-licensed
//! XSD is **not** redistributed; only the schema *shape* is encoded.
//! Coverage is a documented subset projected onto the (unchanged)
//! scalar `FeedbackRecord` — see `docs/auth-messages/emir-auth092.md`.
//!
//! Real EMIR envelope:
//! ```text
//! Document
//! └─ DerivsTradRjctnSttstclRpt   (DerivativesTradeRejectionStatisticalReportV04)
//!    └─ RjctnSttstcs  (choice)
//!       ├─ DataSetActn = "NOTX"          (no-activity / empty report)
//!       └─ Rpt → … RjctnSttstcs → DerivSttstcs  (choice)
//!          ├─ DataSetActn = "NOTX"
//!          └─ DtldSttstcs
//!             └─ TxsRjctnsRsn (0..500000)   ← RECORD
//!                ├─ TxId/UnqIdr/UnqTxIdr (or TxId/UnqIdr/Prtry/Id)
//!                ├─ TxId/RptgTmStmp
//!                ├─ Sts  (ACPT|RJCT|INCF|CRPT|NAUT)
//!                └─ DtldVldtnRule (0..n) : Id [+ Desc]
//! ```
//! `auth.092` carries no "Missing"/"Inaccurate" branch. Every
//! `DtldVldtnRule/Id` is captured into `validation_rule_codes`; the
//! scalar `reason_code` is its first element (kept for compatibility).
//! `ACPT` rows are not feedback and are skipped.
//!
//! ## SFTR `auth.080` — not feedback (handled by `sftr reconcile`)
//!
//! Real `auth.080` is a *reconciliation status advice*, not rejection
//! feedback. The **real `auth.080.001.02`** parser lives in
//! `reconciliation.rs` and is reached via `opendqi sftr reconcile`
//! (see `docs/auth-messages/sftr-auth080.md`). SFTR has **no**
//! rejection-feedback message, so the `SFTR.FBK.*` checks have no real
//! SFTR input. This synthetic SFTR path is retained only as an inert
//! placeholder for the `auth.080.001.01` hand-authored fixture; a
//! real `auth.080.001.02` document routed here trips the
//! unsupported-namespace warning by design.

use std::path::Path;

use chrono::{DateTime, Utc};
use opendqi_core::{DqDimension, DqIssue, FeedbackRecord, FeedbackType, Regime, Severity};
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::NsReader;

use crate::wellformed::check_wellformedness;

/// Outcome of reading one TR feedback XML file.
#[derive(Debug, Default)]
pub struct FeedbackXmlReadOutcome {
    /// Records extracted from the file.
    pub records: Vec<FeedbackRecord>,
    /// File-level data-quality / parse issues (format / namespace).
    pub issues: Vec<DqIssue>,
}

const ISO20022_AUTH_092_NS: &[u8] = b"urn:iso:std:iso:20022:tech:xsd:auth.092.001.04";
const ISO20022_AUTH_080_NS: &[u8] = b"urn:iso:std:iso:20022:tech:xsd:auth.080.001.01";

/// SFTR synthetic per-record wrapper parent.
const STS_PARENT: &str = "Sts";
/// EMIR real per-transaction rejection record element.
const TXS_RJCTNS_RSN: &str = "TxsRjctnsRsn";

/// Read an EMIR `auth.092` rejection-statistics file.
pub fn read_emir_feedback_xml(path: &Path) -> anyhow::Result<FeedbackXmlReadOutcome> {
    read_with_regime(path, Regime::Emir, ISO20022_AUTH_092_NS, "auth.092.001.04")
}

/// Read an SFTR `auth.080` file.
pub fn read_sftr_feedback_xml(path: &Path) -> anyhow::Result<FeedbackXmlReadOutcome> {
    read_with_regime(path, Regime::Sftr, ISO20022_AUTH_080_NS, "auth.080.001.01")
}

fn read_with_regime(
    path: &Path,
    regime: Regime,
    expected_ns: &[u8],
    expected_label: &str,
) -> anyhow::Result<FeedbackXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();
    let (check_wf, check_ns) = match regime {
        Regime::Emir => (
            "EMIR.FMT.XML_NOT_WELLFORMED",
            "EMIR.FMT.XML_UNSUPPORTED_NAMESPACE",
        ),
        Regime::Sftr => (
            "SFTR.FMT.XML_NOT_WELLFORMED",
            "SFTR.FMT.XML_UNSUPPORTED_NAMESPACE",
        ),
    };

    if let Err(err) = check_wellformedness(path) {
        return Ok(FeedbackXmlReadOutcome {
            records: vec![],
            issues: vec![fmt_issue(
                check_wf,
                regime,
                Severity::Critical,
                format!("XML is not well-formed: {}", err.message),
                source_label,
            )],
        });
    }

    match peek_root_namespace(path)? {
        Some(ns) if ns == expected_ns => match regime {
            Regime::Emir => parse_emir_auth092(path),
            Regime::Sftr => parse_sftr_synthetic(path, regime),
        },
        other => {
            let actual = other
                .as_deref()
                .map(|n| String::from_utf8_lossy(n).into_owned())
                .unwrap_or_else(|| "(none)".into());
            Ok(FeedbackXmlReadOutcome {
                records: vec![],
                issues: vec![fmt_issue(
                    check_ns,
                    regime,
                    Severity::Warning,
                    format!(
                        "Root namespace is '{actual}', expected 'urn:iso:std:iso:20022:tech:xsd:{expected_label}'."
                    ),
                    source_label,
                )],
            })
        }
    }
}

fn fmt_issue(
    check_id: &str,
    regime: Regime,
    severity: Severity,
    message: String,
    source_file: String,
) -> DqIssue {
    DqIssue {
        check_id: check_id.into(),
        regime,
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

fn set_dt(dst: &mut Option<DateTime<Utc>>, s: &str) {
    if !s.is_empty() {
        if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
            *dst = Some(dt.with_timezone(&Utc));
        }
    }
}

// ---- EMIR auth.092 (real DATREJ envelope, documented subset) --------

fn parse_emir_auth092(path: &Path) -> anyhow::Result<FeedbackXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();
    let mut reader = NsReader::from_file(path)?;
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut pile: Vec<String> = Vec::new();
    let mut is_leaf: Vec<bool> = Vec::new();
    let mut text_buf = String::new();

    let mut current: Option<FeedbackRecord> = None;
    let mut current_accepted = false;
    let mut rec_depth: Option<usize> = None;
    let mut records: Vec<FeedbackRecord> = Vec::new();
    let mut rec_index: u32 = 0;
    let mut saw_dataset_actn = false;

    loop {
        match reader.read_resolved_event_into(&mut buf)? {
            (_, Event::Start(e)) => {
                let local = local_name(&e);
                push_element(&mut pile, &mut is_leaf, local);
                text_buf.clear();

                if current.is_none()
                    && pile.last().map(String::as_str) == Some(TXS_RJCTNS_RSN)
                    && pile.iter().any(|s| s == "DtldSttstcs")
                {
                    rec_index += 1;
                    current_accepted = false;
                    current = Some(FeedbackRecord {
                        source_file: Some(source_label.clone()),
                        record_id: Some(format!("{source_label}#rjctn-{rec_index}")),
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
                    // DataSetActn (NOTX) sits outside any record.
                    if current.is_none()
                        && pile.last().map(String::as_str) == Some("DataSetActn")
                        && pile.iter().any(|s| s == "RjctnSttstcs")
                    {
                        saw_dataset_actn = true;
                    }
                    if let (Some(rec), Some(rdepth)) = (current.as_mut(), rec_depth) {
                        if pile.len() > rdepth {
                            commit_leaf_emir(rec, &pile[rdepth..], trimmed, &mut current_accepted);
                        }
                    }
                }

                if let Some(rdepth) = rec_depth {
                    if pile.len() == rdepth {
                        if let Some(rec) = current.take() {
                            // ACPT transactions are not feedback rows.
                            if !current_accepted {
                                records.push(rec);
                            }
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
            "EMIR.FMT.FBK_NO_RECORDS",
            Regime::Emir,
            Severity::Info,
            "Rejection-statistics report carries RjctnSttstcs/DataSetActn \
             (no-activity report); zero rejected transactions to evaluate."
                .to_string(),
            source_label,
        ));
    }

    Ok(FeedbackXmlReadOutcome { records, issues })
}

/// Map one leaf (path relative to `TxsRjctnsRsn`) onto the scalar
/// `FeedbackRecord`. Documented lossy projection of the real schema.
fn commit_leaf_emir(rec: &mut FeedbackRecord, rel: &[String], value: &str, accepted: &mut bool) {
    if value.is_empty() {
        return;
    }
    // UTI: TxId/UnqIdr/UnqTxIdr (or proprietary).
    if tail(rel, &["UnqTxIdr"]) || (tail(rel, &["Prtry", "Id"]) && has(rel, "UnqIdr")) {
        if rec.uti.is_none() {
            rec.uti = Some(value.to_owned());
        }
        return;
    }
    if rel.last().map(String::as_str) == Some("RptgTmStmp") {
        set_dt(&mut rec.feedback_timestamp, value);
        return;
    }
    if rel.last().map(String::as_str) == Some("Sts") {
        match value {
            "ACPT" => *accepted = true,
            // RJCT / INCF / CRPT / NAUT are all rejections.
            _ => rec.feedback_type = FeedbackType::Rejected,
        }
        return;
    }
    // Every DtldVldtnRule/Id (auth.092 lists several per transaction);
    // the full faithful list, with `reason_code` = its first element.
    if tail(rel, &["DtldVldtnRule", "Id"]) {
        rec.validation_rule_codes.push(value.to_owned());
        if rec.reason_code.is_none() {
            rec.reason_code = Some(value.to_owned());
        }
        return;
    }
    if tail(rel, &["DtldVldtnRule", "Desc"]) && rec.reason_description.is_none() {
        rec.reason_description = Some(value.to_owned());
    }
}

// ---- SFTR auth.080 (synthetic — unchanged, caveat only) -------------

fn parse_sftr_synthetic(path: &Path, regime: Regime) -> anyhow::Result<FeedbackXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();
    let mut reader = NsReader::from_file(path)?;
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut pile: Vec<String> = Vec::new();
    let mut is_leaf: Vec<bool> = Vec::new();
    let mut text_buf = String::new();

    let mut header_timestamp: Option<DateTime<Utc>> = None;
    let mut current: Option<FeedbackRecord> = None;
    let mut sts_depth: Option<usize> = None;
    let mut records: Vec<FeedbackRecord> = Vec::new();
    let mut sts_index: u32 = 0;

    loop {
        match reader.read_resolved_event_into(&mut buf)? {
            (_, Event::Start(e)) => {
                let local = local_name(&e);
                push_element(&mut pile, &mut is_leaf, local.clone());
                text_buf.clear();

                if current.is_none() && pile.last().map(String::as_str) == Some(STS_PARENT) {
                    sts_index += 1;
                    current = Some(FeedbackRecord {
                        source_file: Some(source_label.clone()),
                        record_id: Some(format!("{source_label}#sts-{sts_index}")),
                        regime,
                        feedback_timestamp: header_timestamp,
                        ..Default::default()
                    });
                    sts_depth = Some(pile.len());
                    continue;
                }

                if let (Some(rec), Some(sdepth)) = (current.as_mut(), sts_depth) {
                    if pile.len() == sdepth + 1 {
                        if let Some(ft) = wrapper_to_feedback_type(&local) {
                            rec.feedback_type = ft;
                        }
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
                    if current.is_none()
                        && pile.ends_with(&["Hdr".into(), "FdbckDtTm".into()])
                        && !trimmed.is_empty()
                    {
                        if let Ok(dt) = DateTime::parse_from_rfc3339(trimmed) {
                            header_timestamp = Some(dt.with_timezone(&Utc));
                        }
                    }
                    if let (Some(rec), Some(sdepth)) = (current.as_mut(), sts_depth) {
                        if pile.len() > sdepth + 1 {
                            let leaf = pile.last().map(String::as_str).unwrap_or("");
                            commit_leaf_sftr(rec, leaf, trimmed);
                        }
                    }
                }

                if let Some(sdepth) = sts_depth {
                    if pile.len() == sdepth {
                        if let Some(mut rec) = current.take() {
                            if rec.feedback_timestamp.is_none() {
                                rec.feedback_timestamp = header_timestamp;
                            }
                            records.push(rec);
                        }
                        sts_depth = None;
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

    Ok(FeedbackXmlReadOutcome {
        records,
        issues: Vec::new(),
    })
}

fn wrapper_to_feedback_type(local: &str) -> Option<FeedbackType> {
    Some(match local {
        "Rjctd" => FeedbackType::Rejected,
        "Mssng" => FeedbackType::Missing,
        "Inaccrt" => FeedbackType::Inaccurate,
        "RcncltnBrk" => FeedbackType::ReconciliationBreak,
        _ => return None,
    })
}

fn commit_leaf_sftr(rec: &mut FeedbackRecord, leaf: &str, value: &str) {
    if value.is_empty() {
        return;
    }
    match leaf {
        "UnqTxIdr" => rec.uti = Some(value.to_owned()),
        "RsnCd" => {
            rec.validation_rule_codes.push(value.to_owned());
            rec.reason_code = Some(value.to_owned());
        }
        "RsnDesc" => rec.reason_description = Some(value.to_owned()),
        "FldNm" => rec.reported_field = Some(value.to_owned()),
        _ => {}
    }
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
        let p =
            std::env::temp_dir().join(format!("opendqi-feedback-{}-{name}", std::process::id()));
        std::fs::write(&p, content).unwrap();
        p
    }

    const REAL_AUTH092: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.092.001.04">
  <DerivsTradRjctnSttstclRpt>
    <RjctnSttstcs>
      <Rpt>
        <RjctnSttstcs>
          <DerivSttstcs>
            <DtldSttstcs>
              <TxsRjctnsRsn>
                <TxId><UnqIdr><UnqTxIdr>OPENDQI-FBK-RJCT-0001</UnqTxIdr></UnqIdr>
                  <RptgTmStmp>2026-05-13T08:00:00Z</RptgTmStmp></TxId>
                <Sts>RJCT</Sts>
                <DtldVldtnRule><Id>VR-0001</Id><Desc>Notional missing</Desc></DtldVldtnRule>
                <DtldVldtnRule><Id>VR-0009</Id></DtldVldtnRule>
              </TxsRjctnsRsn>
              <TxsRjctnsRsn>
                <TxId><UnqIdr><UnqTxIdr>OPENDQI-FBK-ACPT-0002</UnqTxIdr></UnqIdr></TxId>
                <Sts>ACPT</Sts>
              </TxsRjctnsRsn>
            </DtldSttstcs>
          </DerivSttstcs>
        </RjctnSttstcs>
      </Rpt>
    </RjctnSttstcs>
  </DerivsTradRjctnSttstclRpt>
</Document>"#;

    #[test]
    fn parses_real_auth092_subset() {
        let p = write_tmp("emir.xml", REAL_AUTH092);
        let out = read_emir_feedback_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert!(out.issues.is_empty());
        // ACPT row skipped; only the RJCT transaction is a feedback row.
        assert_eq!(out.records.len(), 1);
        let r = &out.records[0];
        assert_eq!(r.uti.as_deref(), Some("OPENDQI-FBK-RJCT-0001"));
        assert_eq!(r.feedback_type, FeedbackType::Rejected);
        // Faithful: every DtldVldtnRule/Id; reason_code = the first.
        assert_eq!(r.validation_rule_codes, vec!["VR-0001", "VR-0009"]);
        assert_eq!(r.reason_code.as_deref(), Some("VR-0001"));
        assert_eq!(r.reason_description.as_deref(), Some("Notional missing"));
        assert!(r.feedback_timestamp.is_some());
    }

    #[test]
    fn emir_empty_report_no_records_info() {
        let body = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.092.001.04">
  <DerivsTradRjctnSttstclRpt>
    <RjctnSttstcs><DataSetActn>NOTX</DataSetActn></RjctnSttstcs>
  </DerivsTradRjctnSttstclRpt>
</Document>"#;
        let p = write_tmp("empty.xml", body);
        let out = read_emir_feedback_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert!(out.records.is_empty());
        assert_eq!(out.issues.len(), 1);
        assert_eq!(out.issues[0].check_id, "EMIR.FMT.FBK_NO_RECORDS");
        assert_eq!(out.issues[0].severity, Severity::Info);
    }

    #[test]
    fn unsupported_namespace_yields_warning() {
        let body = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.030.001.03"/>"#;
        let p = write_tmp("wrong-ns.xml", body);
        let out = read_emir_feedback_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert!(out.records.is_empty());
        assert_eq!(out.issues[0].check_id, "EMIR.FMT.XML_UNSUPPORTED_NAMESPACE");
    }

    #[test]
    fn malformed_xml_yields_critical() {
        let body = br#"<?xml version="1.0"?><Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.092.001.04"><TxsRjctnsRsn"#;
        let p = write_tmp("bad.xml", body);
        let out = read_emir_feedback_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert!(out.records.is_empty());
        assert_eq!(out.issues[0].severity, Severity::Critical);
    }

    // SFTR auth.080 stays on the synthetic shape (caveat only).
    #[test]
    fn parses_sftr_feedback() {
        let body = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.080.001.01">
  <FeedbackToReportingMembers>
    <Sts><Mssng><UnqTxIdr>S-MISSING</UnqTxIdr></Mssng></Sts>
  </FeedbackToReportingMembers>
</Document>"#;
        let p = write_tmp("sftr.xml", body);
        let out = read_sftr_feedback_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert_eq!(out.records.len(), 1);
        assert_eq!(out.records[0].regime, Regime::Sftr);
        assert_eq!(out.records[0].feedback_type, FeedbackType::Missing);
    }
}
