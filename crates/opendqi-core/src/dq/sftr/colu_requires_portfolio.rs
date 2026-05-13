//! SFTR.CON.COLU_REQUIRES_PORTFOLIO — collateral updates must
//! reference a collateral portfolio.

use super::SftrCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};

/// Check implementation.
pub struct SftrColuRequiresPortfolio;

const CHECK_ID: &str = "SFTR.CON.COLU_REQUIRES_PORTFOLIO";

impl SftrCheck for SftrColuRequiresPortfolio {
    fn id(&self) -> &'static str {
        CHECK_ID
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
            .filter(|r| {
                r.action_type
                    .as_deref()
                    .map(|s| s.eq_ignore_ascii_case("COLU"))
                    .unwrap_or(false)
                    && r.collateral_portfolio_code
                        .as_deref()
                        .map(|s| s.trim().is_empty())
                        .unwrap_or(true)
            })
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Sftr,
                severity: Severity::High,
                dimension: DqDimension::Consistency,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("collateral_portfolio_code".into()),
                value: None,
                message: "Action type is COLU but no collateral portfolio code is reported.".into(),
                source_file: r.source_file.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flags_colu_without_portfolio() {
        let r = SftrRecord {
            action_type: Some("COLU".into()),
            collateral_portfolio_code: None,
            ..Default::default()
        };
        assert_eq!(
            SftrColuRequiresPortfolio
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_colu_with_portfolio() {
        let r = SftrRecord {
            action_type: Some("COLU".into()),
            collateral_portfolio_code: Some("PTF-1".into()),
            ..Default::default()
        };
        assert!(SftrColuRequiresPortfolio
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
