//! SFTR margin lending — state-oriented checks. Operate on
//! `SftrTrStateRecord` rows whose `sft_type` is `MGLD`. See
//! `docs/sftr-margin-lending.md`.

use rust_decimal::Decimal;

use crate::dq::sftr::tr_state::SftrTrStateCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord, SftrTrStateRecord};

fn is_mgld(r: &SftrTrStateRecord) -> bool {
    r.sft_type
        .as_deref()
        .map(|s| s.trim().eq_ignore_ascii_case("MGLD"))
        .unwrap_or(false)
}

fn is_outstanding(r: &SftrTrStateRecord) -> bool {
    match r.status.as_deref() {
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

fn issue(
    check_id: &str,
    severity: Severity,
    dimension: DqDimension,
    r: &SftrTrStateRecord,
    field: &str,
    value: Option<String>,
    message: String,
) -> DqIssue {
    DqIssue {
        check_id: check_id.into(),
        regime: Regime::Sftr,
        severity,
        dimension,
        record_id: r.record_id.clone(),
        uti: r.uti.clone(),
        field: Some(field.into()),
        value,
        message,
        source_file: r.source_file.clone(),
    }
}

// -------- SFTR.MSR.MGLD_OUTSTANDING_NEEDS_LOAN --------------------

/// Check implementation.
pub struct SftrMsrMgldOutstandingNeedsLoan;

impl SftrTrStateCheck for SftrMsrMgldOutstandingNeedsLoan {
    fn id(&self) -> &'static str {
        "SFTR.MSR.MGLD_OUTSTANDING_NEEDS_LOAN"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(
        &self,
        records: &[SftrTrStateRecord],
        _prior: &[SftrRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| is_mgld(r) && is_outstanding(r))
            .filter(|r| r.loan_value.is_none())
            .map(|r| {
                issue(
                    self.id(),
                    self.severity(),
                    self.dimension(),
                    r,
                    "loan_value",
                    None,
                    "MGLD SFT outstanding in the TSR has no loan_value.".into(),
                )
            })
            .collect()
    }
}

// -------- SFTR.MSR.MGLD_HAIRCUT_OUT_OF_RANGE ----------------------

/// Check implementation.
pub struct SftrMsrMgldHaircutOutOfRange;

impl SftrTrStateCheck for SftrMsrMgldHaircutOutOfRange {
    fn id(&self) -> &'static str {
        "SFTR.MSR.MGLD_HAIRCUT_OUT_OF_RANGE"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(
        &self,
        records: &[SftrTrStateRecord],
        _prior: &[SftrRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        let mut out = Vec::new();
        for r in records.iter().filter(|r| is_mgld(r)) {
            if let Some(h) = r.haircut {
                if h < Decimal::ZERO || h > Decimal::ONE {
                    out.push(issue(
                        self.id(),
                        self.severity(),
                        self.dimension(),
                        r,
                        "haircut",
                        Some(h.to_string()),
                        format!("MGLD TSR haircut {h} is outside [0, 1]."),
                    ));
                }
            }
        }
        out
    }
}

// -------- SFTR.MSR.MGLD_COLLATERAL_UNDER_LOAN ---------------------

/// Check implementation. Detects under-collateralisation:
/// `collateral_value < loan_value × (1 - haircut)`.
pub struct SftrMsrMgldCollateralUnderLoan;

impl SftrTrStateCheck for SftrMsrMgldCollateralUnderLoan {
    fn id(&self) -> &'static str {
        "SFTR.MSR.MGLD_COLLATERAL_UNDER_LOAN"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(
        &self,
        records: &[SftrTrStateRecord],
        _prior: &[SftrRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        let mut out = Vec::new();
        for r in records.iter().filter(|r| is_mgld(r) && is_outstanding(r)) {
            let (Some(loan), Some(coll)) = (r.loan_value, r.collateral_value) else {
                continue;
            };
            let haircut = r.haircut.unwrap_or(Decimal::ZERO);
            // Sanity: haircut > 1 produces a negative threshold; treat
            // that as out-of-range and skip — covered by HAIRCUT_OUT_OF_RANGE.
            if haircut < Decimal::ZERO || haircut > Decimal::ONE {
                continue;
            }
            let threshold = loan * (Decimal::ONE - haircut);
            if coll < threshold {
                out.push(issue(
                    self.id(),
                    self.severity(),
                    self.dimension(),
                    r,
                    "collateral_value",
                    Some(format!("loan={loan} coll={coll} haircut={haircut}")),
                    format!(
                        "MGLD under-collateralised: collateral {coll} < loan {loan} × (1 - haircut {haircut}) = {threshold}."
                    ),
                ));
            }
        }
        out
    }
}

// -------- SFTR.MSR.MGLD_REUSE_REQUIRES_PORTFOLIO ------------------

/// Check implementation.
pub struct SftrMsrMgldReuseRequiresPortfolio;

impl SftrTrStateCheck for SftrMsrMgldReuseRequiresPortfolio {
    fn id(&self) -> &'static str {
        "SFTR.MSR.MGLD_REUSE_REQUIRES_PORTFOLIO"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(
        &self,
        records: &[SftrTrStateRecord],
        _prior: &[SftrRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| is_mgld(r))
            .filter(|r| r.reuse_indicator == Some(true))
            .filter(|r| {
                r.collateral_portfolio_code
                    .as_deref()
                    .map(|s| s.trim().is_empty())
                    .unwrap_or(true)
            })
            .map(|r| {
                issue(
                    self.id(),
                    self.severity(),
                    self.dimension(),
                    r,
                    "collateral_portfolio_code",
                    None,
                    "MGLD with reuse_indicator=true must report a collateral_portfolio_code."
                        .into(),
                )
            })
            .collect()
    }
}

// -------- SFTR.MSR.MGLD_LOAN_COLL_CURRENCY_MISMATCH ---------------

/// Check implementation.
pub struct SftrMsrMgldLoanCollCurrencyMismatch;

impl SftrTrStateCheck for SftrMsrMgldLoanCollCurrencyMismatch {
    fn id(&self) -> &'static str {
        "SFTR.MSR.MGLD_LOAN_COLL_CURRENCY_MISMATCH"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Validity
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(
        &self,
        records: &[SftrTrStateRecord],
        _prior: &[SftrRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        let mut out = Vec::new();
        for r in records.iter().filter(|r| is_mgld(r)) {
            if let (Some(loan_c), Some(coll_c)) =
                (r.loan_currency.as_deref(), r.collateral_currency.as_deref())
            {
                if !loan_c.eq_ignore_ascii_case(coll_c) {
                    out.push(issue(
                        self.id(),
                        self.severity(),
                        self.dimension(),
                        r,
                        "collateral_currency",
                        Some(format!("loan={loan_c} coll={coll_c}")),
                        format!(
                            "MGLD loan ({loan_c}) and collateral ({coll_c}) currencies differ."
                        ),
                    ));
                }
            }
        }
        out
    }
}

// -------- SFTR.MSR.MGLD_MISSING_ISIN ------------------------------

/// Check implementation.
pub struct SftrMsrMgldMissingIsin;

impl SftrTrStateCheck for SftrMsrMgldMissingIsin {
    fn id(&self) -> &'static str {
        "SFTR.MSR.MGLD_MISSING_ISIN"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(
        &self,
        records: &[SftrTrStateRecord],
        _prior: &[SftrRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| is_mgld(r) && is_outstanding(r))
            .filter(|r| r.collateral_value.is_some())
            .filter(|r| {
                r.collateral_isin
                    .as_deref()
                    .map(|s| s.trim().is_empty())
                    .unwrap_or(true)
            })
            .map(|r| {
                issue(
                    self.id(),
                    self.severity(),
                    self.dimension(),
                    r,
                    "collateral_isin",
                    None,
                    "MGLD outstanding with collateral_value reported has no collateral_isin."
                        .into(),
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> CheckContext {
        CheckContext {
            thresholds: Default::default(),
            today: chrono::NaiveDate::from_ymd_opt(2026, 5, 14).unwrap(),
            now: chrono::DateTime::parse_from_rfc3339("2026-05-14T08:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
        }
    }

    fn mgld() -> SftrTrStateRecord {
        SftrTrStateRecord {
            sft_type: Some("MGLD".into()),
            status: Some("OUTSTANDING".into()),
            ..Default::default()
        }
    }

    #[test]
    fn needs_loan_flags_and_accepts() {
        let mut r = mgld();
        assert_eq!(
            SftrMsrMgldOutstandingNeedsLoan
                .run(&[r.clone()], &[], &ctx())
                .len(),
            1
        );
        r.loan_value = Some(Decimal::from(1000));
        assert!(SftrMsrMgldOutstandingNeedsLoan
            .run(&[r], &[], &ctx())
            .is_empty());
    }

    #[test]
    fn needs_loan_skips_non_mgld_and_non_outstanding() {
        let r = SftrTrStateRecord {
            sft_type: Some("REPO".into()),
            ..Default::default()
        };
        assert!(SftrMsrMgldOutstandingNeedsLoan
            .run(&[r], &[], &ctx())
            .is_empty());
        let r = SftrTrStateRecord {
            sft_type: Some("MGLD".into()),
            status: Some("TERMINATED".into()),
            ..Default::default()
        };
        assert!(SftrMsrMgldOutstandingNeedsLoan
            .run(&[r], &[], &ctx())
            .is_empty());
    }

    #[test]
    fn haircut_out_of_range_flags_and_accepts() {
        let mut r = mgld();
        r.haircut = Some(Decimal::new(15, 1));
        assert_eq!(
            SftrMsrMgldHaircutOutOfRange
                .run(&[r.clone()], &[], &ctx())
                .len(),
            1
        );
        r.haircut = Some(Decimal::new(5, 2));
        assert!(SftrMsrMgldHaircutOutOfRange
            .run(&[r], &[], &ctx())
            .is_empty());
    }

    #[test]
    fn collateral_under_loan_flags_and_accepts() {
        let mut r = mgld();
        r.loan_value = Some(Decimal::from(1000));
        r.haircut = Some(Decimal::new(10, 2)); // 10% — threshold = 900
        r.collateral_value = Some(Decimal::from(800));
        assert_eq!(
            SftrMsrMgldCollateralUnderLoan
                .run(&[r.clone()], &[], &ctx())
                .len(),
            1
        );
        r.collateral_value = Some(Decimal::from(950));
        assert!(SftrMsrMgldCollateralUnderLoan
            .run(&[r], &[], &ctx())
            .is_empty());
    }

    #[test]
    fn reuse_requires_portfolio_flags_and_accepts() {
        let mut r = mgld();
        r.reuse_indicator = Some(true);
        assert_eq!(
            SftrMsrMgldReuseRequiresPortfolio
                .run(&[r.clone()], &[], &ctx())
                .len(),
            1
        );
        r.collateral_portfolio_code = Some("P".into());
        assert!(SftrMsrMgldReuseRequiresPortfolio
            .run(&[r], &[], &ctx())
            .is_empty());
    }

    #[test]
    fn loan_coll_currency_mismatch_flags_and_accepts() {
        let mut r = mgld();
        r.loan_currency = Some("EUR".into());
        r.collateral_currency = Some("USD".into());
        assert_eq!(
            SftrMsrMgldLoanCollCurrencyMismatch
                .run(&[r.clone()], &[], &ctx())
                .len(),
            1
        );
        r.collateral_currency = Some("EUR".into());
        assert!(SftrMsrMgldLoanCollCurrencyMismatch
            .run(&[r], &[], &ctx())
            .is_empty());
    }

    #[test]
    fn missing_isin_flags_and_accepts() {
        let mut r = mgld();
        r.collateral_value = Some(Decimal::from(100));
        assert_eq!(
            SftrMsrMgldMissingIsin.run(&[r.clone()], &[], &ctx()).len(),
            1
        );
        r.collateral_isin = Some("FR0000000001".into());
        assert!(SftrMsrMgldMissingIsin.run(&[r], &[], &ctx()).is_empty());
    }
}
