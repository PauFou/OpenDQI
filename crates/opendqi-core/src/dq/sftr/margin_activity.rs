//! SFTR margin lending — activity-oriented checks. SFTR does not have
//! a dedicated margin-activity ISO 20022 message (no auth.108-style
//! equivalent); margin lending flows inline via `auth.052` with
//! `sft_type=MGLD` or `action_type=MARU`. These checks operate on
//! `SftrRecord` and filter on those discriminants.
//!
//! See `docs/sftr-margin-lending.md` for the product rationale.

use rust_decimal::Decimal;

use crate::dq::{CheckContext, SftrCheck};
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};

fn is_mgld(r: &SftrRecord) -> bool {
    r.sft_type
        .as_deref()
        .map(|s| s.trim().eq_ignore_ascii_case("MGLD"))
        .unwrap_or(false)
}

fn is_maru(r: &SftrRecord) -> bool {
    r.action_type
        .as_deref()
        .map(|a| a.trim().eq_ignore_ascii_case("MARU"))
        .unwrap_or(false)
}

fn is_outstanding(r: &SftrRecord) -> bool {
    r.termination_date.is_none()
}

fn issue(
    check_id: &str,
    severity: Severity,
    dimension: DqDimension,
    r: &SftrRecord,
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
        evidence: Vec::new(),
    }
}

// -------- SFTR.MAR.MGLD_NEEDS_LOAN_VALUE -------------------------

/// Check implementation.
pub struct SftrMarMgldNeedsLoanValue;

impl SftrCheck for SftrMarMgldNeedsLoanValue {
    fn id(&self) -> &'static str {
        "SFTR.MAR.MGLD_NEEDS_LOAN_VALUE"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[SftrRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| is_mgld(r))
            .filter(|r| is_outstanding(r))
            .filter(|r| r.loan_value.is_none())
            .map(|r| {
                issue(
                    self.id(),
                    self.severity(),
                    self.dimension(),
                    r,
                    "loan_value",
                    None,
                    "MGLD outstanding SFT has no loan_value reported.".into(),
                )
            })
            .collect()
    }
}

// -------- SFTR.MAR.MGLD_NEEDS_COLLATERAL --------------------------

/// Check implementation.
pub struct SftrMarMgldNeedsCollateral;

impl SftrCheck for SftrMarMgldNeedsCollateral {
    fn id(&self) -> &'static str {
        "SFTR.MAR.MGLD_NEEDS_COLLATERAL"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[SftrRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| is_mgld(r))
            .filter(|r| r.loan_value.is_some())
            .filter(|r| r.collateral_value.is_none())
            .map(|r| {
                issue(
                    self.id(),
                    self.severity(),
                    self.dimension(),
                    r,
                    "collateral_value",
                    None,
                    "MGLD SFT with a loan_value has no collateral_value reported.".into(),
                )
            })
            .collect()
    }
}

// -------- SFTR.MAR.MARU_REQUIRES_VALUE_OR_HAIRCUT -----------------

/// Check implementation.
pub struct SftrMarMaruRequiresValueOrHaircut;

impl SftrCheck for SftrMarMaruRequiresValueOrHaircut {
    fn id(&self) -> &'static str {
        "SFTR.MAR.MARU_REQUIRES_VALUE_OR_HAIRCUT"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[SftrRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| is_maru(r))
            .filter(|r| {
                r.loan_value.is_none() && r.collateral_value.is_none() && r.haircut.is_none()
            })
            .map(|r| {
                issue(
                    self.id(),
                    self.severity(),
                    self.dimension(),
                    r,
                    "action_type",
                    Some("MARU".into()),
                    "MARU action updates no margin field (loan_value / collateral_value / haircut).".into(),
                )
            })
            .collect()
    }
}

// -------- SFTR.MAR.MARU_REQUIRES_PORTFOLIO ------------------------

/// Check implementation.
pub struct SftrMarMaruRequiresPortfolio;

impl SftrCheck for SftrMarMaruRequiresPortfolio {
    fn id(&self) -> &'static str {
        "SFTR.MAR.MARU_REQUIRES_PORTFOLIO"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[SftrRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| is_maru(r))
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
                    "MARU action without collateral_portfolio_code — portfolio scope is required."
                        .into(),
                )
            })
            .collect()
    }
}

// -------- SFTR.MAR.MGLD_HAIRCUT_OUT_OF_RANGE ----------------------

/// Check implementation.
pub struct SftrMarMgldHaircutOutOfRange;

impl SftrCheck for SftrMarMgldHaircutOutOfRange {
    fn id(&self) -> &'static str {
        "SFTR.MAR.MGLD_HAIRCUT_OUT_OF_RANGE"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[SftrRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
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
                        format!("MGLD haircut {h} is outside [0, 1]."),
                    ));
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dq::CheckContext;

    fn ctx() -> CheckContext {
        CheckContext {
            thresholds: Default::default(),
            today: chrono::NaiveDate::from_ymd_opt(2026, 5, 14).unwrap(),
            now: chrono::DateTime::parse_from_rfc3339("2026-05-14T08:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
        }
    }

    fn mgld() -> SftrRecord {
        SftrRecord {
            sft_type: Some("MGLD".into()),
            ..Default::default()
        }
    }

    #[test]
    fn needs_loan_value_flags_and_accepts() {
        let mut r = mgld();
        assert_eq!(SftrMarMgldNeedsLoanValue.run(&[r.clone()], &ctx()).len(), 1);
        r.loan_value = Some(Decimal::from(1000));
        assert!(SftrMarMgldNeedsLoanValue.run(&[r], &ctx()).is_empty());
    }

    #[test]
    fn needs_loan_value_skips_non_mgld() {
        let r = SftrRecord {
            sft_type: Some("REPO".into()),
            ..Default::default()
        };
        assert!(SftrMarMgldNeedsLoanValue.run(&[r], &ctx()).is_empty());
    }

    #[test]
    fn needs_collateral_flags_and_accepts() {
        let mut r = mgld();
        r.loan_value = Some(Decimal::from(1000));
        assert_eq!(
            SftrMarMgldNeedsCollateral.run(&[r.clone()], &ctx()).len(),
            1
        );
        r.collateral_value = Some(Decimal::from(1100));
        assert!(SftrMarMgldNeedsCollateral.run(&[r], &ctx()).is_empty());
    }

    #[test]
    fn maru_requires_value_or_haircut_flags_and_accepts() {
        let mut r = SftrRecord {
            action_type: Some("MARU".into()),
            ..Default::default()
        };
        assert_eq!(
            SftrMarMaruRequiresValueOrHaircut
                .run(&[r.clone()], &ctx())
                .len(),
            1
        );
        r.haircut = Some(Decimal::new(5, 2));
        assert!(SftrMarMaruRequiresValueOrHaircut
            .run(&[r], &ctx())
            .is_empty());
    }

    #[test]
    fn maru_requires_portfolio_flags_and_accepts() {
        let mut r = SftrRecord {
            action_type: Some("MARU".into()),
            ..Default::default()
        };
        assert_eq!(
            SftrMarMaruRequiresPortfolio.run(&[r.clone()], &ctx()).len(),
            1
        );
        r.collateral_portfolio_code = Some("P".into());
        assert!(SftrMarMaruRequiresPortfolio.run(&[r], &ctx()).is_empty());
    }

    #[test]
    fn maru_requires_portfolio_skips_non_maru() {
        let r = SftrRecord {
            action_type: Some("NEWT".into()),
            ..Default::default()
        };
        assert!(SftrMarMaruRequiresPortfolio.run(&[r], &ctx()).is_empty());
    }

    #[test]
    fn mgld_haircut_out_of_range_flags_and_accepts() {
        let mut r = mgld();
        r.haircut = Some(Decimal::new(15, 1));
        assert_eq!(
            SftrMarMgldHaircutOutOfRange.run(&[r.clone()], &ctx()).len(),
            1
        );
        r.haircut = Some(Decimal::new(5, 2));
        assert!(SftrMarMgldHaircutOutOfRange.run(&[r], &ctx()).is_empty());
    }
}
