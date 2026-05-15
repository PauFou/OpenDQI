//! EMIR Reconciliation Statistics ingestion — ISO 20022 `auth.091`.
//!
//! Element paths are aligned with the real ESMA EMIR REFIT usage
//! guideline `auth.091.001.02_ESMAUG_DATREC_1.0.0`
//! (`DerivativesTradeReconciliationStatisticalReportV02`). The
//! SWIFT-licensed XSD is **not** redistributed; only the schema *shape*
//! is encoded. Coverage is a documented **derive-subset** projected
//! onto the (unchanged) scalar `ReconStatsRecord` — see
//! `docs/auth-messages/emir-auth091.md`.
//!
//! Real envelope:
//! ```text
//! Document
//! └─ DerivsTradRcncltnSttstclRpt  (DerivativesTradeReconciliationStatisticalReportV02)
//!    └─ RcncltnSttstcs  (choice)
//!       ├─ DataSetActn = "NOTX"          (no-activity / empty report)
//!       └─ Rpt (1..n)   one per (RefDt × reconciliation-category cohort)
//!          ├─ RefDt
//!          ├─ RcncltnCtgrs/RptgRqrmnt: Pairg(PARD|UNPR), Rcncltn(RECO|NREC)
//!          │  (or NoRptgRqrmnt — no Pairg/Rcncltn, contributes nothing)
//!          ├─ TtlNbOfTxs                 (cohort total — context only)
//!          └─ TxDtls (0..n)
//!             ├─ CtrPtyId/RptgCtrPty/LEI
//!             ├─ TtlNbOfTxs              ← per-pair count for THIS cohort
//!             └─ RcncltnRpt (1..n)       (not modelled)
//! ```
//! `auth.091` carries **no explicit pairing/recon rate** and **no
//! outstanding-paired/unpaired count**. OpenDQI accumulates the
//! per-pair `TxDtls/TtlNbOfTxs` by cohort `Pairg`/`Rcncltn` and emits
//! one `ReconStatsRecord` per counterparty LEI with the rates
//! **derived** (`paired/(paired+unpaired)` etc). `outstanding_*` stays
//! `None` → `EMIR.RST.OUTSTANDING_UNPAIRED_HIGH` is unreachable.

use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;

use chrono::NaiveDate;
use opendqi_core::{DqDimension, DqIssue, ReconStatsRecord, Regime, Severity};
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::NsReader;
use rust_decimal::Decimal;

use crate::wellformed::check_wellformedness;

/// Outcome of reading one auth.091 XML file.
#[derive(Debug, Default)]
pub struct ReconStatsXmlReadOutcome {
    /// Records extracted from the file (one per counterparty LEI).
    pub records: Vec<ReconStatsRecord>,
    /// File-level data-quality / parse issues (format / namespace).
    pub issues: Vec<DqIssue>,
}

const ISO20022_AUTH_091_NS: &[u8] = b"urn:iso:std:iso:20022:tech:xsd:auth.091.001.02";

/// Read an EMIR `auth.091` Reconciliation Statistics file.
pub fn read_emir_recon_stats_xml(path: &Path) -> anyhow::Result<ReconStatsXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();

    if let Err(err) = check_wellformedness(path) {
        return Ok(ReconStatsXmlReadOutcome {
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
        Some(ns) if ns == ISO20022_AUTH_091_NS => parse(path),
        other => {
            let actual = other
                .as_deref()
                .map(|n| String::from_utf8_lossy(n).into_owned())
                .unwrap_or_else(|| "(none)".into());
            Ok(ReconStatsXmlReadOutcome {
                records: vec![],
                issues: vec![fmt_issue(
                    "EMIR.FMT.XML_UNSUPPORTED_NAMESPACE",
                    Severity::Warning,
                    format!(
                        "Root namespace is '{actual}', expected 'urn:iso:std:iso:20022:tech:xsd:auth.091.001.02'."
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

/// True when `pile` ends with `suffix`.
fn tail(pile: &[String], suffix: &[&str]) -> bool {
    pile.len() >= suffix.len()
        && pile[pile.len() - suffix.len()..]
            .iter()
            .zip(suffix)
            .all(|(a, b)| a == *b)
}

/// True when `seg` appears anywhere in `pile`.
fn has(pile: &[String], seg: &str) -> bool {
    pile.iter().any(|s| s == seg)
}

/// Per-counterparty accumulation of cohort transaction counts.
#[derive(Default)]
struct Accum {
    paired: Decimal,
    unpaired: Decimal,
    reconciled: Decimal,
    unreconciled: Decimal,
    ref_date: Option<NaiveDate>,
}

fn parse(path: &Path) -> anyhow::Result<ReconStatsXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();
    let mut reader = NsReader::from_file(path)?;
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut pile: Vec<String> = Vec::new();
    let mut is_leaf: Vec<bool> = Vec::new();
    let mut text_buf = String::new();

    // Cohort state (one `Rpt` = one RefDt × category cohort).
    let mut rpt_depth: Option<usize> = None;
    let mut cohort_ref_date: Option<NaiveDate> = None;
    let mut cohort_pairg: Option<String> = None;
    let mut cohort_rcncltn: Option<String> = None;
    // Current TxDtls (counterparty pair within the cohort).
    let mut txd_depth: Option<usize> = None;
    let mut txd_lei: Option<String> = None;
    let mut txd_count: Option<Decimal> = None;

    let mut acc: BTreeMap<String, Accum> = BTreeMap::new();
    let mut saw_dataset_actn = false;

    loop {
        match reader.read_resolved_event_into(&mut buf)? {
            (_, Event::Start(e)) => {
                let local = local_name(&e);
                push_element(&mut pile, &mut is_leaf, local);
                text_buf.clear();

                if pile.last().map(String::as_str) == Some("Rpt")
                    && pile.iter().any(|s| s == "RcncltnSttstcs")
                    && rpt_depth.is_none()
                {
                    rpt_depth = Some(pile.len());
                    cohort_ref_date = None;
                    cohort_pairg = None;
                    cohort_rcncltn = None;
                } else if pile.last().map(String::as_str) == Some("TxDtls")
                    && rpt_depth.is_some()
                    && txd_depth.is_none()
                {
                    txd_depth = Some(pile.len());
                    txd_lei = None;
                    txd_count = None;
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
                    let v = text_buf.trim();
                    if !v.is_empty() {
                        if rpt_depth.is_none() && tail(&pile, &["RcncltnSttstcs", "DataSetActn"]) {
                            saw_dataset_actn = true;
                        } else if rpt_depth.is_some() {
                            let leaf = pile.last().map(String::as_str).unwrap_or("");
                            if txd_depth.is_some() {
                                // Per-pair leaves.
                                if leaf == "LEI" && has(&pile, "RptgCtrPty") {
                                    txd_lei = Some(v.to_owned());
                                } else if leaf == "TtlNbOfTxs" {
                                    txd_count = Decimal::from_str(v).ok();
                                }
                            } else {
                                // Cohort-level leaves.
                                match leaf {
                                    "RefDt" => {
                                        cohort_ref_date =
                                            NaiveDate::parse_from_str(v, "%Y-%m-%d").ok();
                                    }
                                    "Pairg" if has(&pile, "RptgRqrmnt") => {
                                        cohort_pairg = Some(v.to_owned());
                                    }
                                    "Rcncltn" if has(&pile, "RptgRqrmnt") => {
                                        cohort_rcncltn = Some(v.to_owned());
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }

                // Leaving a TxDtls: fold its count into the LEI accumulator.
                if let Some(td) = txd_depth {
                    if pile.len() == td {
                        if let (Some(lei), Some(cnt)) = (txd_lei.take(), txd_count.take()) {
                            let a = acc.entry(lei).or_default();
                            if a.ref_date.is_none() {
                                a.ref_date = cohort_ref_date;
                            }
                            match cohort_pairg.as_deref() {
                                Some("PARD") => a.paired += cnt,
                                Some("UNPR") => a.unpaired += cnt,
                                _ => {}
                            }
                            match cohort_rcncltn.as_deref() {
                                Some("RECO") => a.reconciled += cnt,
                                Some("NREC") => a.unreconciled += cnt,
                                _ => {}
                            }
                        }
                        txd_depth = None;
                    }
                }
                // Leaving the Rpt cohort.
                if let Some(rd) = rpt_depth {
                    if pile.len() == rd {
                        rpt_depth = None;
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

    let mut records: Vec<ReconStatsRecord> = Vec::new();
    for (i, (lei, a)) in acc.into_iter().enumerate() {
        let pair_total = a.paired + a.unpaired;
        let recon_total = a.reconciled + a.unreconciled;
        records.push(ReconStatsRecord {
            source_file: Some(source_label.clone()),
            record_id: Some(format!("{source_label}#rst-{}", i + 1)),
            regime: Regime::Emir,
            reporting_date: a.ref_date,
            counterparty_lei: Some(lei),
            pairing_rate: (pair_total > Decimal::ZERO).then(|| a.paired / pair_total),
            recon_rate: (recon_total > Decimal::ZERO).then(|| a.reconciled / recon_total),
            // No real-schema source in auth.091.
            outstanding_paired: None,
            outstanding_unpaired: None,
            raw_fields: Default::default(),
        });
    }

    let mut issues = Vec::new();
    if records.is_empty() && saw_dataset_actn {
        issues.push(fmt_issue(
            "EMIR.FMT.RST_NO_RECORDS",
            Severity::Info,
            "Reconciliation-statistics report carries RcncltnSttstcs/DataSetActn \
             (no-activity report); zero reconciliation cohorts to evaluate."
                .to_string(),
            source_label,
        ));
    }

    Ok(ReconStatsXmlReadOutcome { records, issues })
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
        let p = std::env::temp_dir().join(format!("opendqi-rst-{}-{name}", std::process::id()));
        std::fs::write(&p, content).unwrap();
        p
    }

    // Two cohorts for LEI-A: PARD/RECO 90 txns + UNPR/NREC 30 txns ->
    // pairing 90/120 = 0.75, recon 90/120 = 0.75.
    const REAL_ENVELOPE: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.091.001.02">
  <DerivsTradRcncltnSttstclRpt>
    <RcncltnSttstcs>
      <Rpt>
        <RefDt>2026-05-13</RefDt>
        <RcncltnCtgrs><RptgRqrmnt>
          <RptgTp>BOTH</RptgTp><Pairg>PARD</Pairg><Rcncltn>RECO</Rcncltn>
          <ValtnRcncltn>RECO</ValtnRcncltn><Rvvd>false</Rvvd><FrthrMod>false</FrthrMod>
        </RptgRqrmnt></RcncltnCtgrs>
        <TtlNbOfTxs>90</TtlNbOfTxs>
        <TxDtls>
          <CtrPtyId><RptgCtrPty><LEI>LEIAAAAAAAAAAAAAAAAA1</LEI></RptgCtrPty></CtrPtyId>
          <TtlNbOfTxs>90</TtlNbOfTxs>
          <RcncltnRpt><TxId><UnqIdr><UnqTxIdr>U-1</UnqTxIdr></UnqIdr></TxId></RcncltnRpt>
        </TxDtls>
      </Rpt>
      <Rpt>
        <RefDt>2026-05-13</RefDt>
        <RcncltnCtgrs><RptgRqrmnt>
          <RptgTp>BOTH</RptgTp><Pairg>UNPR</Pairg><Rcncltn>NREC</Rcncltn>
          <ValtnRcncltn>NREC</ValtnRcncltn><Rvvd>false</Rvvd><FrthrMod>false</FrthrMod>
        </RptgRqrmnt></RcncltnCtgrs>
        <TtlNbOfTxs>30</TtlNbOfTxs>
        <TxDtls>
          <CtrPtyId><RptgCtrPty><LEI>LEIAAAAAAAAAAAAAAAAA1</LEI></RptgCtrPty></CtrPtyId>
          <TtlNbOfTxs>30</TtlNbOfTxs>
          <RcncltnRpt><TxId><UnqIdr><UnqTxIdr>U-2</UnqTxIdr></UnqIdr></TxId></RcncltnRpt>
        </TxDtls>
      </Rpt>
    </RcncltnSttstcs>
  </DerivsTradRcncltnSttstclRpt>
</Document>"#;

    #[test]
    fn derives_rates_from_cohort_counts() {
        let p = write_tmp("real.xml", REAL_ENVELOPE);
        let out = read_emir_recon_stats_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert!(out.issues.is_empty());
        assert_eq!(out.records.len(), 1);
        let r = &out.records[0];
        assert_eq!(r.counterparty_lei.as_deref(), Some("LEIAAAAAAAAAAAAAAAAA1"));
        assert_eq!(
            r.reporting_date.map(|d| d.to_string()).as_deref(),
            Some("2026-05-13")
        );
        // 90 paired / 120 total = 0.75 ; 90 reconciled / 120 = 0.75
        assert_eq!(r.pairing_rate.unwrap().to_string(), "0.75");
        assert_eq!(r.recon_rate.unwrap().to_string(), "0.75");
        assert!(r.outstanding_unpaired.is_none(), "no source in auth.091");
    }

    #[test]
    fn empty_report_no_records_info() {
        let body = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.091.001.02">
  <DerivsTradRcncltnSttstclRpt>
    <RcncltnSttstcs><DataSetActn>NOTX</DataSetActn></RcncltnSttstcs>
  </DerivsTradRcncltnSttstclRpt>
</Document>"#;
        let p = write_tmp("empty.xml", body);
        let out = read_emir_recon_stats_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert!(out.records.is_empty());
        assert_eq!(out.issues.len(), 1);
        assert_eq!(out.issues[0].check_id, "EMIR.FMT.RST_NO_RECORDS");
        assert_eq!(out.issues[0].severity, Severity::Info);
    }

    #[test]
    fn unsupported_namespace_yields_warning() {
        let body = br#"<?xml version="1.0"?><Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.030.001.03"/>"#;
        let p = write_tmp("wrong.xml", body);
        let out = read_emir_recon_stats_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert!(out.records.is_empty());
        assert_eq!(out.issues[0].check_id, "EMIR.FMT.XML_UNSUPPORTED_NAMESPACE");
    }
}
