//! EMIR.COMP.COLLATERAL_PORTFOLIO_REQUIRED_FOR_FULL — fully-collateralised
//! trades must reference a collateral portfolio code.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct CollateralPortfolioRequiredForFull;

const CHECK_ID: &str = "EMIR.COMP.COLLATERAL_PORTFOLIO_REQUIRED_FOR_FULL";

impl Check for CollateralPortfolioRequiredForFull {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| {
                let is_full = r.collateralisation_category.as_deref()
                    .map(|s| s.eq_ignore_ascii_case("FLCL"))
                    .unwrap_or(false);
                is_full
                    && r.collateral_portfolio_code
                        .as_deref()
                        .map(str::trim)
                        .unwrap_or("")
                        .is_empty()
            })
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Emir,
                severity: Severity::High,
                dimension: DqDimension::Completeness,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("collateral_portfolio_code".into()),
                value: None,
                message: "Collateralisation category is FLCL but no collateral portfolio code is reported.".into(),
                source_file: r.source_file.clone(),
                evidence: Vec::new(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flags_flcl_without_portfolio() {
        let r = EmirRecord {
            collateralisation_category: Some("FLCL".into()),
            collateral_portfolio_code: None,
            ..Default::default()
        };
        assert_eq!(
            CollateralPortfolioRequiredForFull
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_partial_without_portfolio() {
        let r = EmirRecord {
            collateralisation_category: Some("PRCL".into()),
            collateral_portfolio_code: None,
            ..Default::default()
        };
        assert!(CollateralPortfolioRequiredForFull
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
