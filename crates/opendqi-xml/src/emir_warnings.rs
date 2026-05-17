//! EMIR Data-Quality Warnings ingestion — ISO 20022 `auth.106`.
//!
//! Element paths are aligned with the real ESMA EMIR REFIT usage
//! guideline `auth.106.001.01_ESMAUG_DATWRN_1.1.0`
//! (`DerivativesTradeWarningsReportV01`). The SWIFT-licensed XSD is
//! **not** redistributed; only the schema *shape* is encoded. Coverage
//! is a documented **derive-subset** projected onto the (unchanged)
//! scalar `TradeWarningsRecord` — see
//! `docs/auth-messages/emir-auth106.md`.
//!
//! Real envelope:
//! ```text
//! Document
//! └─ DerivsTradWrnngsRpt  (DerivativesTradeWarningsReportV01)
//!    └─ WrnngsSttstcs  (StatisticsPerCounterparty16Choice — choice)
//!       ├─ DataSetActn = "NOTX"          (no-activity / empty report)
//!       └─ Rpt (DetailedStatisticsPerCounterparty17)
//!          ├─ RefDt  (ISODate)
//!          ├─ MssngValtn  (choice: DataSetActn | Rpt):
//!          │     NbOfOutsdngDerivs, NbOfOutsdngDerivsWthNoValtn,
//!          │     NbOfOutsdngDerivsWthOutdtdValtn  [+ Wrnngs(0..n) — deferred]
//!          ├─ MssngMrgnInf (choice: DataSetActn | Rpt):
//!          │     NbOfOutsdngDerivs, NbOfOutsdngDerivsWthNoMrgnInf,
//!          │     NbOfOutsdngDerivsWthOutdtdMrgnInf
//!          └─ AbnrmlVals  (choice: DataSetActn | Rpt):
//!                NbOfDerivsRptd, NbOfDerivsRptdWthOtlrs
//! ```
//! OpenDQI models the **report-level aggregate** (one
//! `TradeWarningsRecord` per `RefDt`, `counterparty_lei` always `None`,
//! rates derived from the counts) AND the **per-counterparty
//! aggregate** (one `WarningsCounterpartyRecord` per `(RefDt, CtrPty
//! LEI)`, merging the three `MssngValtn`/`MssngMrgnInf`/`AbnrmlVals`
//! `Wrnngs` blocks for that LEI) AND the **per-UTI** level (one
//! `WarningsTransactionRecord` per `Wrnngs/TxDtls` — each transaction
//! the TR explicitly flagged for missing valuation / missing margin /
//! abnormal value).

use std::path::Path;

use chrono::NaiveDate;
use opendqi_core::{
    DqDimension, DqIssue, Regime, Severity, TradeWarningsRecord, WarningsCounterpartyRecord,
    WarningsTransactionRecord,
};
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::NsReader;
use rust_decimal::Decimal;
use std::collections::BTreeMap;

use crate::wellformed::check_wellformedness;

/// Outcome of reading one auth.106 XML file.
#[derive(Debug, Default)]
pub struct WarningsXmlReadOutcome {
    /// Report-level records (one per reference-date report).
    pub records: Vec<TradeWarningsRecord>,
    /// Per-counterparty records (one per `(RefDt, CtrPty LEI)`), from
    /// the `Wrnngs` breakdown. Empty when the file carries no `Wrnngs`.
    pub counterparty_records: Vec<WarningsCounterpartyRecord>,
    /// Per-UTI records (one per `Wrnngs/TxDtls` — each transaction the
    /// TR flagged). Empty when the file carries no `TxDtls`.
    pub transaction_records: Vec<WarningsTransactionRecord>,
    /// File-level data-quality / parse issues (format / namespace).
    pub issues: Vec<DqIssue>,
}

const ISO20022_AUTH_106_NS: &[u8] = b"urn:iso:std:iso:20022:tech:xsd:auth.106.001.01";

/// Read an EMIR `auth.106` Data-Quality Warnings file.
pub fn read_emir_warnings_xml(path: &Path) -> anyhow::Result<WarningsXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();

    if let Err(err) = check_wellformedness(path) {
        return Ok(WarningsXmlReadOutcome {
            records: vec![],
            counterparty_records: vec![],
            transaction_records: vec![],
            issues: vec![fmt_issue(
                "EMIR.FMT.XML_NOT_WELLFORMED",
                Severity::Critical,
                format!("XML is not well-formed: {}", err.message),
                source_label,
            )],
        });
    }

    match peek_root_namespace(path)? {
        Some(ns) if ns == ISO20022_AUTH_106_NS => parse(path),
        other => {
            let actual = other
                .as_deref()
                .map(|n| String::from_utf8_lossy(n).into_owned())
                .unwrap_or_else(|| "(none)".into());
            Ok(WarningsXmlReadOutcome {
                records: vec![],
                counterparty_records: vec![],
                transaction_records: vec![],
                issues: vec![fmt_issue(
                    "EMIR.FMT.XML_UNSUPPORTED_NAMESPACE",
                    Severity::Warning,
                    format!(
                        "Root namespace is '{actual}', expected 'urn:iso:std:iso:20022:tech:xsd:auth.106.001.01'."
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

/// Report-level accumulation for one `RefDt` report.
#[derive(Default)]
struct Accum {
    ref_date: Option<NaiveDate>,
    outsdng_valtn: Option<i64>,
    missing_valtn: Option<i64>,
    outdated_valtn: Option<i64>,
    outsdng_mrgn: Option<i64>,
    missing_mrgn: Option<i64>,
    outdated_mrgn: Option<i64>,
    derivs_rptd: Option<i64>,
    abnormal: Option<i64>,
}

fn rate(num: Option<i64>, den: Option<i64>) -> Option<Decimal> {
    match (num, den) {
        (Some(n), Some(d)) if d > 0 => Some(Decimal::from(n) / Decimal::from(d)),
        _ => None,
    }
}

fn finalize(acc: Accum, source_label: &str, idx: usize) -> TradeWarningsRecord {
    TradeWarningsRecord {
        source_file: Some(source_label.to_owned()),
        record_id: Some(format!("{source_label}#wrn-{}", idx + 1)),
        regime: Regime::Emir,
        reporting_date: acc.ref_date,
        counterparty_lei: None,
        missing_valuation_rate: rate(acc.missing_valtn, acc.outsdng_valtn),
        outdated_valuation_rate: rate(acc.outdated_valtn, acc.outsdng_valtn),
        missing_margin_rate: rate(acc.missing_mrgn, acc.outsdng_mrgn),
        outdated_margin_rate: rate(acc.outdated_mrgn, acc.outsdng_mrgn),
        abnormal_values_rate: rate(acc.abnormal, acc.derivs_rptd),
        outstanding_derivatives: acc.outsdng_valtn,
        missing_valuation: acc.missing_valtn,
        outdated_valuation: acc.outdated_valtn,
        outstanding_derivatives_margin: acc.outsdng_mrgn,
        missing_margin_info: acc.missing_mrgn,
        outdated_margin_info: acc.outdated_mrgn,
        derivatives_reported: acc.derivs_rptd,
        abnormal_values: acc.abnormal,
        raw_fields: Default::default(),
    }
}

/// Per-counterparty accumulation for one `Wrnngs` LEI within a
/// `RefDt` report. The three `MssngValtn` / `MssngMrgnInf` /
/// `AbnrmlVals` `Wrnngs` blocks for the same LEI merge into one of
/// these (each block contributes a disjoint set of count categories).
#[derive(Default)]
struct CpAccum {
    outsdng_valtn: Option<i64>,
    missing_valtn: Option<i64>,
    outdated_valtn: Option<i64>,
    outsdng_mrgn: Option<i64>,
    missing_mrgn: Option<i64>,
    outdated_mrgn: Option<i64>,
    derivs_rptd: Option<i64>,
    abnormal: Option<i64>,
}

impl CpAccum {
    /// Merge another block's counts in (later `Some` wins; the three
    /// blocks set disjoint fields so there is no real conflict).
    fn merge(&mut self, o: CpAccum) {
        self.outsdng_valtn = o.outsdng_valtn.or(self.outsdng_valtn);
        self.missing_valtn = o.missing_valtn.or(self.missing_valtn);
        self.outdated_valtn = o.outdated_valtn.or(self.outdated_valtn);
        self.outsdng_mrgn = o.outsdng_mrgn.or(self.outsdng_mrgn);
        self.missing_mrgn = o.missing_mrgn.or(self.missing_mrgn);
        self.outdated_mrgn = o.outdated_mrgn.or(self.outdated_mrgn);
        self.derivs_rptd = o.derivs_rptd.or(self.derivs_rptd);
        self.abnormal = o.abnormal.or(self.abnormal);
    }
}

fn finalize_cp(
    lei: String,
    acc: CpAccum,
    ref_date: Option<NaiveDate>,
    source_label: &str,
    seq: usize,
) -> WarningsCounterpartyRecord {
    WarningsCounterpartyRecord {
        source_file: Some(source_label.to_owned()),
        record_id: Some(format!("{source_label}#wrn-cp-{seq}")),
        regime: Regime::Emir,
        reporting_date: ref_date,
        counterparty_lei: Some(lei),
        missing_valuation_rate: rate(acc.missing_valtn, acc.outsdng_valtn),
        outdated_valuation_rate: rate(acc.outdated_valtn, acc.outsdng_valtn),
        missing_margin_rate: rate(acc.missing_mrgn, acc.outsdng_mrgn),
        outdated_margin_rate: rate(acc.outdated_mrgn, acc.outsdng_mrgn),
        abnormal_values_rate: rate(acc.abnormal, acc.derivs_rptd),
        outstanding_derivatives: acc.outsdng_valtn,
        missing_valuation: acc.missing_valtn,
        outdated_valuation: acc.outdated_valtn,
        outstanding_derivatives_margin: acc.outsdng_mrgn,
        missing_margin_info: acc.missing_mrgn,
        outdated_margin_info: acc.outdated_mrgn,
        derivatives_reported: acc.derivs_rptd,
        abnormal_values: acc.abnormal,
        raw_fields: Default::default(),
    }
}

fn parse(path: &Path) -> anyhow::Result<WarningsXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();
    let mut reader = NsReader::from_file(path)?;
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut pile: Vec<String> = Vec::new();
    let mut is_leaf: Vec<bool> = Vec::new();
    let mut text_buf = String::new();
    let mut attrs_buf: Vec<(String, String)> = Vec::new();

    let mut rpt_depth: Option<usize> = None;
    let mut acc = Accum::default();
    let mut records: Vec<TradeWarningsRecord> = Vec::new();
    let mut saw_dataset_actn = false;

    // Per-counterparty `Wrnngs` accumulation, keyed by CtrPty LEI
    // within the current `RefDt` report. `cur_cp`/`cur_cp_lei` hold
    // the block currently being read; on the `Wrnngs` close it is
    // merged into `cp_acc` (one LEI appears once per sub-report).
    let mut cp_acc: BTreeMap<String, CpAccum> = BTreeMap::new();
    let mut cur_cp: Option<CpAccum> = None;
    let mut cur_cp_lei: Option<String> = None;
    let mut wrnngs_depth: Option<usize> = None;
    let mut cp_seq: usize = 0;
    let mut counterparty_records: Vec<WarningsCounterpartyRecord> = Vec::new();

    // Per-UTI `Wrnngs/TxDtls` accumulation — one record per flagged
    // transaction. `cur_tx` is the TxDtls currently being read;
    // pushed on the `TxDtls` close (LEI still in scope).
    let mut txd_depth: Option<usize> = None;
    let mut cur_tx: Option<WarningsTransactionRecord> = None;
    let mut tx_seq: usize = 0;
    let mut transaction_records: Vec<WarningsTransactionRecord> = Vec::new();

    loop {
        match reader.read_resolved_event_into(&mut buf)? {
            (_, Event::Start(e)) => {
                let local = local_name(&e);
                push_element(&mut pile, &mut is_leaf, local);
                text_buf.clear();
                // Attributes belong to the element just opened; a leaf
                // element's Start immediately precedes its Text/End
                // (no nested element), so at the leaf-commit below
                // `attrs_buf` holds exactly that leaf's attributes
                // (e.g. the `Ccy` on `<Amt Ccy="EUR">`).
                attrs_buf = collect_attrs(&e);

                // The OUTER per-RefDt Rpt (DetailedStatisticsPerCounterparty17).
                // Inner category `Rpt`s reuse the name but never reopen
                // because `rpt_depth` is already `Some`.
                if pile.last().map(String::as_str) == Some("Rpt")
                    && has(&pile, "WrnngsSttstcs")
                    && rpt_depth.is_none()
                {
                    rpt_depth = Some(pile.len());
                    acc = Accum::default();
                    cp_acc.clear();
                }

                // Open a per-counterparty `Wrnngs` block (only inside
                // the outer report; `TxDtls` is deferred — ignored).
                if pile.last().map(String::as_str) == Some("Wrnngs")
                    && rpt_depth.is_some()
                    && wrnngs_depth.is_none()
                {
                    wrnngs_depth = Some(pile.len());
                    cur_cp = Some(CpAccum::default());
                    cur_cp_lei = None;
                }

                // Open a per-UTI `TxDtls` (inside a `Wrnngs`). The
                // enclosing CtrPty LEI was already read into
                // `cur_cp_lei` (CtrPtyId precedes TxDtls in document
                // order); `acc.ref_date` is set (RefDt precedes the
                // sub-reports).
                if pile.last().map(String::as_str) == Some("TxDtls")
                    && wrnngs_depth.is_some()
                    && txd_depth.is_none()
                {
                    txd_depth = Some(pile.len());
                    let category = if has(&pile, "MssngValtn") {
                        "MissingValuation"
                    } else if has(&pile, "MssngMrgnInf") {
                        "MissingMargin"
                    } else if has(&pile, "AbnrmlVals") {
                        "AbnormalValue"
                    } else {
                        "Unknown"
                    };
                    cur_tx = Some(WarningsTransactionRecord {
                        source_file: Some(source_label.clone()),
                        regime: Regime::Emir,
                        reporting_date: acc.ref_date,
                        counterparty_lei: cur_cp_lei.clone(),
                        warning_category: Some(category.to_owned()),
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
                if leaf_now {
                    let v = text_buf.trim();
                    if !v.is_empty() {
                        if rpt_depth.is_none() && tail(&pile, &["WrnngsSttstcs", "DataSetActn"]) {
                            saw_dataset_actn = true;
                        } else if rpt_depth.is_some() && !has(&pile, "Wrnngs") {
                            // Report-level aggregate only — never the
                            // per-counterparty `Wrnngs` breakdown.
                            let leaf = pile.last().map(String::as_str).unwrap_or("");
                            let n = || v.parse::<i64>().ok();
                            match leaf {
                                "RefDt" => {
                                    acc.ref_date = NaiveDate::parse_from_str(v, "%Y-%m-%d").ok();
                                }
                                "NbOfOutsdngDerivs" if has(&pile, "MssngValtn") => {
                                    acc.outsdng_valtn = n();
                                }
                                "NbOfOutsdngDerivs" if has(&pile, "MssngMrgnInf") => {
                                    acc.outsdng_mrgn = n();
                                }
                                "NbOfOutsdngDerivsWthNoValtn" => acc.missing_valtn = n(),
                                "NbOfOutsdngDerivsWthOutdtdValtn" => acc.outdated_valtn = n(),
                                "NbOfOutsdngDerivsWthNoMrgnInf" => acc.missing_mrgn = n(),
                                "NbOfOutsdngDerivsWthOutdtdMrgnInf" => acc.outdated_mrgn = n(),
                                "NbOfDerivsRptd" => acc.derivs_rptd = n(),
                                "NbOfDerivsRptdWthOtlrs" => acc.abnormal = n(),
                                _ => {}
                            }
                        } else if rpt_depth.is_some()
                            && has(&pile, "Wrnngs")
                            && !has(&pile, "TxDtls")
                        {
                            // Per-counterparty `Wrnngs` aggregate. The
                            // per-UTI `TxDtls` level stays deferred
                            // (guarded out above).
                            let leaf = pile.last().map(String::as_str).unwrap_or("");
                            let n = || v.parse::<i64>().ok();
                            if leaf == "LEI" && has(&pile, "CtrPtyId") && has(&pile, "RptgCtrPty") {
                                cur_cp_lei = Some(v.to_owned());
                            } else if let Some(cp) = cur_cp.as_mut() {
                                match leaf {
                                    "NbOfOutsdngDerivs" if has(&pile, "MssngValtn") => {
                                        cp.outsdng_valtn = n();
                                    }
                                    "NbOfOutsdngDerivs" if has(&pile, "MssngMrgnInf") => {
                                        cp.outsdng_mrgn = n();
                                    }
                                    "NbOfOutsdngDerivsWthNoValtn" => cp.missing_valtn = n(),
                                    "NbOfOutsdngDerivsWthOutdtdValtn" => cp.outdated_valtn = n(),
                                    "NbOfOutsdngDerivsWthNoMrgnInf" => cp.missing_mrgn = n(),
                                    "NbOfOutsdngDerivsWthOutdtdMrgnInf" => cp.outdated_mrgn = n(),
                                    "NbOfDerivsRptd" => cp.derivs_rptd = n(),
                                    "NbOfDerivsRptdWthOtlrs" => cp.abnormal = n(),
                                    _ => {}
                                }
                            }
                        } else if let (Some(td), Some(tx)) = (txd_depth, cur_tx.as_mut()) {
                            // Per-UTI `Wrnngs/TxDtls`. UTI + other-CP
                            // promoted to typed fields; the rest of the
                            // heterogeneous context kept verbatim in
                            // `raw_fields`, keyed by the path from
                            // `TxDtls` down (the `emir_msr.rs` idiom).
                            let leaf = pile.last().map(String::as_str).unwrap_or("");
                            // `TxId/UnqIdr/UnqTxIdr` (the UTI) or, when
                            // proprietary, `TxId/UnqIdr/Prtry/Id`.
                            let is_uti = leaf == "UnqTxIdr"
                                || (leaf == "Id" && has(&pile, "UnqIdr") && has(&pile, "Prtry"));
                            if is_uti && tx.uti.is_none() {
                                tx.uti = Some(v.to_owned());
                            } else if leaf == "LEI"
                                && has(&pile, "OthrCtrPty")
                                && tx.other_counterparty.is_none()
                            {
                                tx.other_counterparty = Some(v.to_owned());
                            } else {
                                let key = pile[td - 1..].join("/");
                                tx.raw_fields
                                    .entry(key)
                                    .or_insert_with(|| encode_value(v, &attrs_buf));
                            }
                        }
                    }
                }

                // Close the current `TxDtls`: emit the per-UTI record
                // (LEI / RefDt still in scope — Wrnngs not yet closed).
                if pile.last().map(String::as_str) == Some("TxDtls")
                    && Some(pile.len()) == txd_depth
                {
                    if let Some(mut tx) = cur_tx.take() {
                        tx_seq += 1;
                        tx.record_id = Some(format!("{source_label}#wrn-tx-{tx_seq}"));
                        transaction_records.push(tx);
                    }
                    txd_depth = None;
                }

                // Close the current `Wrnngs` block: merge its counts
                // into the per-LEI accumulator (one LEI recurs across
                // the three sub-reports).
                if pile.last().map(String::as_str) == Some("Wrnngs")
                    && Some(pile.len()) == wrnngs_depth
                {
                    if let (Some(lei), Some(cp)) = (cur_cp_lei.take(), cur_cp.take()) {
                        cp_acc.entry(lei).or_default().merge(cp);
                    }
                    cur_cp = None;
                    cur_cp_lei = None;
                    wrnngs_depth = None;
                }

                if let Some(rd) = rpt_depth {
                    if pile.len() == rd {
                        let ref_date = acc.ref_date;
                        let idx = records.len();
                        records.push(finalize(std::mem::take(&mut acc), &source_label, idx));
                        for (lei, cp) in std::mem::take(&mut cp_acc) {
                            cp_seq += 1;
                            counterparty_records.push(finalize_cp(
                                lei,
                                cp,
                                ref_date,
                                &source_label,
                                cp_seq,
                            ));
                        }
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

    let mut issues = Vec::new();
    if records.is_empty() && saw_dataset_actn {
        issues.push(fmt_issue(
            "EMIR.FMT.WRN_NO_RECORDS",
            Severity::Info,
            "Warnings report carries WrnngsSttstcs/DataSetActn \
             (no-activity report); zero warning statistics to evaluate."
                .to_string(),
            source_label,
        ));
    }

    Ok(WarningsXmlReadOutcome {
        records,
        counterparty_records,
        transaction_records,
        issues,
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

/// Collect an element's attributes as `(local-name, value)` pairs.
/// Verbatim copy of the `emir/iso20022.rs` idiom for cross-parser
/// consistency.
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

/// Encode a leaf value with its attributes: bare `text` when there are
/// none, else `text|key=value|…` (e.g. `1000.00|Ccy=EUR`). Verbatim
/// copy of the `emir/iso20022.rs` `raw_fields` catch-all idiom.
fn encode_value(text: &str, attrs: &[(String, String)]) -> String {
    if attrs.is_empty() {
        return text.to_owned();
    }
    let mut out = String::from(text);
    for (k, v) in attrs {
        out.push('|');
        out.push_str(k);
        out.push('=');
        out.push_str(v);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_tmp(name: &str, content: &[u8]) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("opendqi-wrn-{}-{name}", std::process::id()));
        std::fs::write(&p, content).unwrap();
        p
    }

    const REAL_ENVELOPE: &[u8] = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.106.001.01">
  <DerivsTradWrnngsRpt>
    <WrnngsSttstcs>
      <Rpt>
        <RefDt>2026-05-13</RefDt>
        <MssngValtn><Rpt>
          <NbOfOutsdngDerivs>1000</NbOfOutsdngDerivs>
          <NbOfOutsdngDerivsWthNoValtn>80</NbOfOutsdngDerivsWthNoValtn>
          <NbOfOutsdngDerivsWthOutdtdValtn>20</NbOfOutsdngDerivsWthOutdtdValtn>
          <Wrnngs>
            <CtrPtyId><RptgCtrPty><LEI>LEIAAAAAAAAAAAAAAAAA1</LEI></RptgCtrPty></CtrPtyId>
            <NbOfOutsdngDerivs>999999</NbOfOutsdngDerivs>
            <NbOfOutsdngDerivsWthNoValtn>999999</NbOfOutsdngDerivsWthNoValtn>
            <TxDtls>
              <TxId>
                <OthrCtrPty><Lgl><LEI>OTHRBBBBBBBBBBBBBBB2</LEI></Lgl></OthrCtrPty>
                <UnqIdr><UnqTxIdr>UTIWRNTX0000000000001</UnqTxIdr></UnqIdr>
              </TxId>
              <ValtnAmt><Amt><Amt Ccy="USD">123456.78</Amt></Amt></ValtnAmt>
              <ValtnTmStmp>2026-05-12T09:00:00Z</ValtnTmStmp>
            </TxDtls>
          </Wrnngs>
        </Rpt></MssngValtn>
        <MssngMrgnInf><Rpt>
          <NbOfOutsdngDerivs>500</NbOfOutsdngDerivs>
          <NbOfOutsdngDerivsWthNoMrgnInf>10</NbOfOutsdngDerivsWthNoMrgnInf>
          <NbOfOutsdngDerivsWthOutdtdMrgnInf>5</NbOfOutsdngDerivsWthOutdtdMrgnInf>
        </Rpt></MssngMrgnInf>
        <AbnrmlVals><Rpt>
          <NbOfDerivsRptd>2000</NbOfDerivsRptd>
          <NbOfDerivsRptdWthOtlrs>40</NbOfDerivsRptdWthOtlrs>
        </Rpt></AbnrmlVals>
      </Rpt>
    </WrnngsSttstcs>
  </DerivsTradWrnngsRpt>
</Document>"#;

    #[test]
    fn derives_warning_rates_report_level() {
        let p = write_tmp("real.xml", REAL_ENVELOPE);
        let out = read_emir_warnings_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert!(out.issues.is_empty());
        assert_eq!(out.records.len(), 1);
        let r = &out.records[0];
        assert_eq!(
            r.reporting_date.map(|d| d.to_string()).as_deref(),
            Some("2026-05-13")
        );
        // Per-counterparty Wrnngs values (999999) must NOT leak into
        // the report-level aggregate.
        assert_eq!(r.outstanding_derivatives, Some(1000));
        assert_eq!(r.missing_valuation, Some(80));
        assert_eq!(r.outdated_valuation, Some(20));
        assert_eq!(r.outstanding_derivatives_margin, Some(500));
        assert_eq!(r.missing_margin_info, Some(10));
        assert_eq!(r.derivatives_reported, Some(2000));
        assert_eq!(r.abnormal_values, Some(40));
        assert_eq!(
            r.counterparty_lei, None,
            "report-level aggregate carries no LEI"
        );
        assert_eq!(r.missing_valuation_rate.unwrap().to_string(), "0.08");
        assert_eq!(r.outdated_valuation_rate.unwrap().to_string(), "0.02");
        assert_eq!(r.missing_margin_rate.unwrap().to_string(), "0.02");
        assert_eq!(r.abnormal_values_rate.unwrap().to_string(), "0.02");

        // The per-counterparty `Wrnngs` block IS now modelled
        // (separately from the report-level aggregate above).
        assert_eq!(out.counterparty_records.len(), 1);
        let cp = &out.counterparty_records[0];
        assert_eq!(
            cp.counterparty_lei.as_deref(),
            Some("LEIAAAAAAAAAAAAAAAAA1")
        );
        assert_eq!(
            cp.reporting_date.map(|d| d.to_string()).as_deref(),
            Some("2026-05-13")
        );
        assert_eq!(cp.outstanding_derivatives, Some(999999));
        assert_eq!(cp.missing_valuation, Some(999999));
        assert_eq!(cp.missing_valuation_rate.unwrap().to_string(), "1");

        // The per-UTI `TxDtls` IS now modelled — and its leaves never
        // leaked into the per-CP counts above (still 999999).
        assert_eq!(out.transaction_records.len(), 1);
        let tx = &out.transaction_records[0];
        assert_eq!(tx.warning_category.as_deref(), Some("MissingValuation"));
        assert_eq!(tx.uti.as_deref(), Some("UTIWRNTX0000000000001"));
        assert_eq!(
            tx.counterparty_lei.as_deref(),
            Some("LEIAAAAAAAAAAAAAAAAA1")
        );
        assert_eq!(
            tx.other_counterparty.as_deref(),
            Some("OTHRBBBBBBBBBBBBBBB2")
        );
        assert_eq!(
            tx.reporting_date.map(|d| d.to_string()).as_deref(),
            Some("2026-05-13")
        );
        assert_eq!(
            tx.raw_fields.get("TxDtls/ValtnTmStmp").map(String::as_str),
            Some("2026-05-12T09:00:00Z")
        );
        // The `Ccy` attribute on the amount leaf is preserved
        // alongside the value via the `text|key=value` encoding.
        assert_eq!(
            tx.raw_fields
                .get("TxDtls/ValtnAmt/Amt/Amt")
                .map(String::as_str),
            Some("123456.78|Ccy=USD")
        );
    }

    #[test]
    fn empty_report_no_records_info() {
        let body = br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.106.001.01">
  <DerivsTradWrnngsRpt>
    <WrnngsSttstcs><DataSetActn>NOTX</DataSetActn></WrnngsSttstcs>
  </DerivsTradWrnngsRpt>
</Document>"#;
        let p = write_tmp("empty.xml", body);
        let out = read_emir_warnings_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert!(out.records.is_empty());
        assert_eq!(out.issues.len(), 1);
        assert_eq!(out.issues[0].check_id, "EMIR.FMT.WRN_NO_RECORDS");
        assert_eq!(out.issues[0].severity, Severity::Info);
    }

    #[test]
    fn unsupported_namespace_yields_warning() {
        let body = br#"<?xml version="1.0"?><Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.030.001.03"/>"#;
        let p = write_tmp("wrong.xml", body);
        let out = read_emir_warnings_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert!(out.records.is_empty());
        assert_eq!(out.issues[0].check_id, "EMIR.FMT.XML_UNSUPPORTED_NAMESPACE");
    }
}
