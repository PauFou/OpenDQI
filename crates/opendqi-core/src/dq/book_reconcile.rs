//! Book-vs-TSR reconciliation — pure functions shared by the CLI
//! (`opendqi {emir,sftr} book-reconcile`) and the local web UI.
//!
//! Compares a firm's internal booking export against the Trade
//! Repository's Trade State Report and emits `*.BREC.*` issues for
//! UTIs present on only one side and field-level mismatches
//! (notional / loan / collateral / currency / valuation / maturity /
//! status).

use std::collections::BTreeMap;

use rust_decimal::Decimal;

use crate::model::{
    DqDimension, DqIssue, EmirRecord, EvidenceItem, Regime, Severity, SftrRecord,
    SftrTrStateRecord, TrStateRecord,
};

/// Relative tolerance (percent) for the EMIR book-vs-TSR valuation
/// comparison.
pub const BOOK_VALUATION_TOLERANCE_PCT: f64 = 1.0;
/// Relative tolerance (percent) for the SFTR loan / collateral
/// comparison.
pub const SFTR_BOOK_VALUE_TOLERANCE_PCT: f64 = 1.0;

fn uti_key(s: &Option<String>) -> Option<&str> {
    s.as_deref().map(str::trim).filter(|t| !t.is_empty())
}

fn within_tolerance(book: Decimal, tsr: Decimal, tol_pct: f64) -> bool {
    if book == tsr {
        return true;
    }
    let book_f = book.to_string().parse::<f64>().unwrap_or(f64::NAN);
    let tsr_f = tsr.to_string().parse::<f64>().unwrap_or(f64::NAN);
    if !book_f.is_finite() || !tsr_f.is_finite() {
        return false;
    }
    if tsr_f.abs() < f64::EPSILON {
        return book_f.abs() < f64::EPSILON;
    }
    let diff_pct = ((book_f - tsr_f).abs() / tsr_f.abs()) * 100.0;
    diff_pct <= tol_pct
}

/// EMIR valuation tolerance check (1% relative).
pub fn valuation_within_tolerance(book: Decimal, tsr: Decimal) -> bool {
    within_tolerance(book, tsr, BOOK_VALUATION_TOLERANCE_PCT)
}

/// SFTR loan / collateral tolerance check (1% relative).
pub fn value_within_tolerance(book: Decimal, tsr: Decimal) -> bool {
    within_tolerance(book, tsr, SFTR_BOOK_VALUE_TOLERANCE_PCT)
}

fn status_is_outstanding(status: Option<&str>) -> bool {
    match status {
        None => true,
        Some(s) => {
            let s = s.trim();
            s.is_empty()
                || s.eq_ignore_ascii_case("OUTSTANDING")
                || s.eq_ignore_ascii_case("ACTIVE")
                || s.eq_ignore_ascii_case("LIVE")
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn brec_issue(
    check_id: &str,
    regime: Regime,
    severity: Severity,
    dimension: DqDimension,
    uti: &str,
    record_id: Option<String>,
    source_file: Option<String>,
    field: Option<String>,
    value: Option<String>,
    message: String,
) -> DqIssue {
    DqIssue {
        check_id: check_id.into(),
        regime,
        severity,
        dimension,
        record_id,
        uti: Some(uti.to_owned()),
        field,
        value,
        message,
        source_file,
        evidence: Vec::new(),
    }
}

fn brec_evidence(field: &str, book_value: &str, tsr_value: &str) -> EvidenceItem {
    EvidenceItem {
        field: field.into(),
        before: Some(book_value.to_owned()),
        after: Some(tsr_value.to_owned()),
        source_line: None,
    }
}

/// Reconcile an EMIR firm book against an `auth.107` TSR snapshot.
pub fn compute_book_reconcile_issues(book: &[EmirRecord], tsr: &[TrStateRecord]) -> Vec<DqIssue> {
    let book_by_uti: BTreeMap<&str, &EmirRecord> = book
        .iter()
        .filter_map(|r| uti_key(&r.uti).map(|u| (u, r)))
        .collect();
    let tsr_by_uti: BTreeMap<&str, &TrStateRecord> = tsr
        .iter()
        .filter_map(|r| uti_key(&r.uti).map(|u| (u, r)))
        .collect();

    let mut all_utis: Vec<&str> = book_by_uti
        .keys()
        .chain(tsr_by_uti.keys())
        .copied()
        .collect();
    all_utis.sort();
    all_utis.dedup();

    let mut out = Vec::new();
    for uti in all_utis {
        match (book_by_uti.get(uti), tsr_by_uti.get(uti)) {
            (Some(b), None) => out.push(brec_issue(
                "EMIR.BREC.IN_BOOK_NOT_IN_TSR",
                Regime::Emir,
                Severity::High,
                DqDimension::Consistency,
                uti,
                b.record_id.clone(),
                b.source_file.clone(),
                Some("uti".into()),
                None,
                format!(
                    "UTI {uti} appears in the firm's book but is absent from the TR State Report."
                ),
            )),
            (None, Some(t)) => {
                if !status_is_outstanding(t.status.as_deref()) {
                    continue;
                }
                out.push(brec_issue(
                    "EMIR.BREC.IN_TSR_NOT_IN_BOOK",
                    Regime::Emir,
                    Severity::High,
                    DqDimension::Consistency,
                    uti,
                    t.record_id.clone(),
                    t.source_file.clone(),
                    Some("uti".into()),
                    None,
                    format!(
                        "UTI {uti} is outstanding at the TR but is absent from the firm's book."
                    ),
                ));
            }
            (Some(b), Some(t)) => {
                if let (Some(bn), Some(tn)) = (b.notional_amount, t.notional_amount) {
                    if bn != tn {
                        let mut iss = brec_issue(
                            "EMIR.BREC.NOTIONAL_MISMATCH",
                            Regime::Emir,
                            Severity::High,
                            DqDimension::Accuracy,
                            uti,
                            b.record_id.clone(),
                            b.source_file.clone(),
                            Some("notional_amount".into()),
                            Some(format!("book={bn} tsr={tn}")),
                            format!("Notional mismatch on UTI {uti}: book {bn} vs TSR {tn}."),
                        );
                        iss.evidence.push(brec_evidence(
                            "notional_amount",
                            &bn.to_string(),
                            &tn.to_string(),
                        ));
                        out.push(iss);
                    }
                }
                if let (Some(bc), Some(tc)) = (
                    b.notional_currency.as_deref(),
                    t.notional_currency.as_deref(),
                ) {
                    if !bc.eq_ignore_ascii_case(tc) {
                        let mut iss = brec_issue(
                            "EMIR.BREC.NOTIONAL_CURRENCY_MISMATCH",
                            Regime::Emir,
                            Severity::Warning,
                            DqDimension::Validity,
                            uti,
                            b.record_id.clone(),
                            b.source_file.clone(),
                            Some("notional_currency".into()),
                            Some(format!("book={bc} tsr={tc}")),
                            format!(
                                "Notional currency mismatch on UTI {uti}: book {bc} vs TSR {tc}."
                            ),
                        );
                        iss.evidence
                            .push(brec_evidence("notional_currency", bc, tc));
                        out.push(iss);
                    }
                }
                if let (Some(bv), Some(tv)) = (b.valuation_amount, t.valuation_amount) {
                    if !valuation_within_tolerance(bv, tv) {
                        let mut iss = brec_issue(
                            "EMIR.BREC.VALUATION_MISMATCH",
                            Regime::Emir,
                            Severity::Warning,
                            DqDimension::Accuracy,
                            uti,
                            b.record_id.clone(),
                            b.source_file.clone(),
                            Some("valuation_amount".into()),
                            Some(format!("book={bv} tsr={tv}")),
                            format!(
                                "Valuation mismatch on UTI {uti}: book {bv} vs TSR {tv} (tolerance {BOOK_VALUATION_TOLERANCE_PCT}%)."
                            ),
                        );
                        iss.evidence.push(brec_evidence(
                            "valuation_amount",
                            &bv.to_string(),
                            &tv.to_string(),
                        ));
                        out.push(iss);
                    }
                }
                if let (Some(bm), Some(tm)) = (b.maturity_date, t.maturity_date) {
                    if bm != tm {
                        let mut iss = brec_issue(
                            "EMIR.BREC.MATURITY_MISMATCH",
                            Regime::Emir,
                            Severity::High,
                            DqDimension::Accuracy,
                            uti,
                            b.record_id.clone(),
                            b.source_file.clone(),
                            Some("maturity_date".into()),
                            Some(format!("book={bm} tsr={tm}")),
                            format!("Maturity mismatch on UTI {uti}: book {bm} vs TSR {tm}."),
                        );
                        iss.evidence.push(brec_evidence(
                            "maturity_date",
                            &bm.to_string(),
                            &tm.to_string(),
                        ));
                        out.push(iss);
                    }
                }
                let book_active = b.termination_date.is_none();
                let tsr_terminated = t
                    .status
                    .as_deref()
                    .map(|s| s.trim().eq_ignore_ascii_case("TERMINATED"))
                    .unwrap_or(false);
                if book_active && tsr_terminated {
                    out.push(brec_issue(
                        "EMIR.BREC.STATUS_MISMATCH",
                        Regime::Emir,
                        Severity::Warning,
                        DqDimension::Consistency,
                        uti,
                        b.record_id.clone(),
                        b.source_file.clone(),
                        Some("status".into()),
                        Some("book=active tsr=TERMINATED".into()),
                        format!(
                            "Status mismatch on UTI {uti}: book has no termination but TSR reports the trade as TERMINATED."
                        ),
                    ));
                }
            }
            (None, None) => unreachable!(),
        }
    }
    out
}

/// Reconcile an SFTR firm book against an `auth.079` TSR snapshot.
pub fn compute_sftr_book_reconcile_issues(
    book: &[SftrRecord],
    tsr: &[SftrTrStateRecord],
) -> Vec<DqIssue> {
    let book_by_uti: BTreeMap<&str, &SftrRecord> = book
        .iter()
        .filter_map(|r| uti_key(&r.uti).map(|u| (u, r)))
        .collect();
    let tsr_by_uti: BTreeMap<&str, &SftrTrStateRecord> = tsr
        .iter()
        .filter_map(|r| uti_key(&r.uti).map(|u| (u, r)))
        .collect();

    let mut all_utis: Vec<&str> = book_by_uti
        .keys()
        .chain(tsr_by_uti.keys())
        .copied()
        .collect();
    all_utis.sort();
    all_utis.dedup();

    let mut out = Vec::new();
    for uti in all_utis {
        match (book_by_uti.get(uti), tsr_by_uti.get(uti)) {
            (Some(b), None) => out.push(brec_issue(
                "SFTR.BREC.IN_BOOK_NOT_IN_TSR",
                Regime::Sftr,
                Severity::High,
                DqDimension::Consistency,
                uti,
                b.record_id.clone(),
                b.source_file.clone(),
                Some("uti".into()),
                None,
                format!(
                    "SFT UTI {uti} appears in the firm's book but is absent from the SFTR TSR."
                ),
            )),
            (None, Some(t)) => {
                if !status_is_outstanding(t.status.as_deref()) {
                    continue;
                }
                out.push(brec_issue(
                    "SFTR.BREC.IN_TSR_NOT_IN_BOOK",
                    Regime::Sftr,
                    Severity::High,
                    DqDimension::Consistency,
                    uti,
                    t.record_id.clone(),
                    t.source_file.clone(),
                    Some("uti".into()),
                    None,
                    format!(
                        "SFT UTI {uti} is outstanding at the TR but is absent from the firm's book."
                    ),
                ));
            }
            (Some(b), Some(t)) => {
                if let (Some(bn), Some(tn)) = (b.loan_value, t.loan_value) {
                    if !value_within_tolerance(bn, tn) {
                        out.push(brec_issue(
                            "SFTR.BREC.LOAN_MISMATCH",
                            Regime::Sftr,
                            Severity::High,
                            DqDimension::Accuracy,
                            uti,
                            b.record_id.clone(),
                            b.source_file.clone(),
                            Some("loan_value".into()),
                            Some(format!("book={bn} tsr={tn}")),
                            format!(
                                "Loan value mismatch on SFT UTI {uti}: book {bn} vs TSR {tn} (tolerance {SFTR_BOOK_VALUE_TOLERANCE_PCT}%)."
                            ),
                        ));
                    }
                }
                if let (Some(bc), Some(tc)) =
                    (b.loan_currency.as_deref(), t.loan_currency.as_deref())
                {
                    if !bc.eq_ignore_ascii_case(tc) {
                        out.push(brec_issue(
                            "SFTR.BREC.LOAN_CURRENCY_MISMATCH",
                            Regime::Sftr,
                            Severity::Warning,
                            DqDimension::Validity,
                            uti,
                            b.record_id.clone(),
                            b.source_file.clone(),
                            Some("loan_currency".into()),
                            Some(format!("book={bc} tsr={tc}")),
                            format!(
                                "Loan currency mismatch on SFT UTI {uti}: book {bc} vs TSR {tc}."
                            ),
                        ));
                    }
                }
                if let (Some(bv), Some(tv)) = (b.collateral_value, t.collateral_value) {
                    if !value_within_tolerance(bv, tv) {
                        out.push(brec_issue(
                            "SFTR.BREC.COLLATERAL_MISMATCH",
                            Regime::Sftr,
                            Severity::Warning,
                            DqDimension::Accuracy,
                            uti,
                            b.record_id.clone(),
                            b.source_file.clone(),
                            Some("collateral_value".into()),
                            Some(format!("book={bv} tsr={tv}")),
                            format!(
                                "Collateral value mismatch on SFT UTI {uti}: book {bv} vs TSR {tv} (tolerance {SFTR_BOOK_VALUE_TOLERANCE_PCT}%)."
                            ),
                        ));
                    }
                }
                if let (Some(bm), Some(tm)) = (b.maturity_date, t.maturity_date) {
                    if bm != tm {
                        out.push(brec_issue(
                            "SFTR.BREC.MATURITY_MISMATCH",
                            Regime::Sftr,
                            Severity::High,
                            DqDimension::Accuracy,
                            uti,
                            b.record_id.clone(),
                            b.source_file.clone(),
                            Some("maturity_date".into()),
                            Some(format!("book={bm} tsr={tm}")),
                            format!("Maturity mismatch on SFT UTI {uti}: book {bm} vs TSR {tm}."),
                        ));
                    }
                }
                let book_active = b.termination_date.is_none();
                let tsr_terminated = t
                    .status
                    .as_deref()
                    .map(|s| s.trim().eq_ignore_ascii_case("TERMINATED"))
                    .unwrap_or(false);
                if book_active && tsr_terminated {
                    out.push(brec_issue(
                        "SFTR.BREC.STATUS_MISMATCH",
                        Regime::Sftr,
                        Severity::Warning,
                        DqDimension::Consistency,
                        uti,
                        b.record_id.clone(),
                        b.source_file.clone(),
                        Some("status".into()),
                        Some("book=active tsr=TERMINATED".into()),
                        format!(
                            "Status mismatch on SFT UTI {uti}: book has no termination but TSR reports the SFT as TERMINATED."
                        ),
                    ));
                }
            }
            (None, None) => unreachable!(),
        }
    }
    out
}
