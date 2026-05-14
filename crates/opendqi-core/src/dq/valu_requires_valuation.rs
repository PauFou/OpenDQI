//! EMIR.CON.VALU_REQUIRES_VALUATION — a valuation update must carry
//! a valuation amount.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct ValuRequiresValuation;

const CHECK_ID: &str = "EMIR.CON.VALU_REQUIRES_VALUATION";

impl Check for ValuRequiresValuation {
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
                    .map(|s| s.eq_ignore_ascii_case("VALU"))
                    .unwrap_or(false)
                    && r.valuation_amount.is_none()
            })
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Emir,
                severity: Severity::High,
                dimension: DqDimension::Consistency,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("valuation_amount".into()),
                value: None,
                message: "Action type is VALU but no valuation amount is reported.".into(),
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
    fn flags_valu_without_amount() {
        let r = EmirRecord {
            action_type: Some("VALU".into()),
            valuation_amount: None,
            ..Default::default()
        };
        assert_eq!(
            ValuRequiresValuation
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_valu_with_amount() {
        let r = EmirRecord {
            action_type: Some("VALU".into()),
            valuation_amount: Some(Decimal::from(1)),
            ..Default::default()
        };
        assert!(ValuRequiresValuation
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
