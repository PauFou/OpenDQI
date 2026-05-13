//! EMIR.CON.MTM_CHANGE_REQUIRES_VALUATION — an MtM change implies a
//! valuation amount is also reported.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct MtmChangeRequiresValuation;

const CHECK_ID: &str = "EMIR.CON.MTM_CHANGE_REQUIRES_VALUATION";

impl Check for MtmChangeRequiresValuation {
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
            .filter(|r| r.mtm_value_change.is_some() && r.valuation_amount.is_none())
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Emir,
                severity: Severity::Warning,
                dimension: DqDimension::Consistency,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("valuation_amount".into()),
                value: None,
                message: "MtM value change is set but no valuation amount is reported.".into(),
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
    fn flags_change_without_valuation() {
        let r = EmirRecord {
            mtm_value_change: Some(Decimal::from(1)),
            valuation_amount: None,
            ..Default::default()
        };
        assert_eq!(
            MtmChangeRequiresValuation
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_with_valuation() {
        let r = EmirRecord {
            mtm_value_change: Some(Decimal::from(1)),
            valuation_amount: Some(Decimal::from(100)),
            ..Default::default()
        };
        assert!(MtmChangeRequiresValuation
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
