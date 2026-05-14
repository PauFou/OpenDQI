//! EMIR.CON.IM_NEEDS_COLLATERAL_PORTFOLIO — initial margin must be
//! tied to a collateral portfolio.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct ImNeedsCollateralPortfolio;

const CHECK_ID: &str = "EMIR.CON.IM_NEEDS_COLLATERAL_PORTFOLIO";

impl Check for ImNeedsCollateralPortfolio {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| {
                r.initial_margin_posted.is_some()
                    && r.collateral_portfolio_code
                        .as_deref()
                        .map(str::trim)
                        .unwrap_or("")
                        .is_empty()
            })
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Emir,
                severity: Severity::Warning,
                dimension: DqDimension::Consistency,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("collateral_portfolio_code".into()),
                value: None,
                message:
                    "Initial margin posted is set but no collateral portfolio code is reported."
                        .into(),
                source_file: r.source_file.clone(),
                evidence: Vec::new(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    #[test]
    fn flags_when_im_without_portfolio() {
        let r = EmirRecord {
            initial_margin_posted: Some(Decimal::from(100)),
            collateral_portfolio_code: None,
            ..Default::default()
        };
        assert_eq!(
            ImNeedsCollateralPortfolio
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_with_portfolio() {
        let r = EmirRecord {
            initial_margin_posted: Some(Decimal::from(100)),
            collateral_portfolio_code: Some("PORT".into()),
            ..Default::default()
        };
        assert!(ImNeedsCollateralPortfolio
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
