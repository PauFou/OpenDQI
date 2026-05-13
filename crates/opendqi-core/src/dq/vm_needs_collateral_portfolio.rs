//! EMIR.CON.VM_NEEDS_COLLATERAL_PORTFOLIO — variation margin must be
//! tied to a collateral portfolio.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct VmNeedsCollateralPortfolio;

const CHECK_ID: &str = "EMIR.CON.VM_NEEDS_COLLATERAL_PORTFOLIO";

impl Check for VmNeedsCollateralPortfolio {
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
                r.variation_margin_posted.is_some()
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
                    "Variation margin posted is set but no collateral portfolio code is reported."
                        .into(),
                source_file: r.source_file.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    #[test]
    fn flags_when_vm_without_portfolio() {
        let r = EmirRecord {
            variation_margin_posted: Some(Decimal::from(10)),
            collateral_portfolio_code: None,
            ..Default::default()
        };
        assert_eq!(
            VmNeedsCollateralPortfolio
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_with_portfolio() {
        let r = EmirRecord {
            variation_margin_posted: Some(Decimal::from(10)),
            collateral_portfolio_code: Some("PORT".into()),
            ..Default::default()
        };
        assert!(VmNeedsCollateralPortfolio
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
