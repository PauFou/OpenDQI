//! EMIR.CON.MARU_REQUIRES_PORTFOLIO — margin updates should reference
//! their collateral portfolio.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct MaruRequiresPortfolio;

const CHECK_ID: &str = "EMIR.CON.MARU_REQUIRES_PORTFOLIO";

impl Check for MaruRequiresPortfolio {
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
                r.action_type
                    .as_deref()
                    .map(|s| s.eq_ignore_ascii_case("MARU"))
                    .unwrap_or(false)
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
                message: "Action type is MARU but no collateral portfolio code is reported.".into(),
                source_file: r.source_file.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flags_maru_without_portfolio() {
        let r = EmirRecord {
            action_type: Some("MARU".into()),
            collateral_portfolio_code: None,
            ..Default::default()
        };
        assert_eq!(
            MaruRequiresPortfolio
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_maru_with_portfolio() {
        let r = EmirRecord {
            action_type: Some("MARU".into()),
            collateral_portfolio_code: Some("PORT".into()),
            ..Default::default()
        };
        assert!(MaruRequiresPortfolio
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
