//! EMIR.CON.POSC_REQUIRES_PORTFOLIO — position-component reports must
//! reference a collateral portfolio.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct PoscRequiresPortfolio;

const CHECK_ID: &str = "EMIR.CON.POSC_REQUIRES_PORTFOLIO";

impl Check for PoscRequiresPortfolio {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| {
                r.action_type
                    .as_deref()
                    .map(|s| s.eq_ignore_ascii_case("POSC"))
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
                severity: Severity::High,
                dimension: DqDimension::Consistency,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("collateral_portfolio_code".into()),
                value: None,
                message: "Action type is POSC but no collateral portfolio code is reported.".into(),
                source_file: r.source_file.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flags_posc_without_portfolio() {
        let r = EmirRecord {
            action_type: Some("POSC".into()),
            collateral_portfolio_code: None,
            ..Default::default()
        };
        assert_eq!(
            PoscRequiresPortfolio
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_posc_with_portfolio() {
        let r = EmirRecord {
            action_type: Some("POSC".into()),
            collateral_portfolio_code: Some("PORT".into()),
            ..Default::default()
        };
        assert!(PoscRequiresPortfolio
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
