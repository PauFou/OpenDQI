//! EMIR.CON.ETRM_REQUIRES_VALUATION — an early-termination report
//! should carry a final valuation.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct EtrmRequiresValuation;

const CHECK_ID: &str = "EMIR.CON.ETRM_REQUIRES_VALUATION";

impl Check for EtrmRequiresValuation {
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
                    .map(|s| s.eq_ignore_ascii_case("ETRM"))
                    .unwrap_or(false)
                    && r.valuation_amount.is_none()
            })
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Emir,
                severity: Severity::Warning,
                dimension: DqDimension::Consistency,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("valuation_amount".into()),
                value: None,
                message: "Action type is ETRM but no final valuation is reported.".into(),
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
    fn flags_etrm_without_valuation() {
        let r = EmirRecord {
            action_type: Some("ETRM".into()),
            valuation_amount: None,
            ..Default::default()
        };
        assert_eq!(
            EtrmRequiresValuation
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_etrm_with_valuation() {
        let r = EmirRecord {
            action_type: Some("ETRM".into()),
            valuation_amount: Some(Decimal::from(1)),
            ..Default::default()
        };
        assert!(EtrmRequiresValuation
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
