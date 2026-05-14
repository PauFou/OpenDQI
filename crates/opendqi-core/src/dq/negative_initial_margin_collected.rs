//! EMIR.ACC.NEGATIVE_INITIAL_MARGIN_COLLECTED — initial margin collected ≥ 0.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};
use rust_decimal::Decimal;

/// Check implementation.
pub struct NegativeInitialMarginCollected;

const CHECK_ID: &str = "EMIR.ACC.NEGATIVE_INITIAL_MARGIN_COLLECTED";

impl Check for NegativeInitialMarginCollected {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| {
                r.initial_margin_collected
                    .map(|v| v < Decimal::ZERO)
                    .unwrap_or(false)
            })
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Emir,
                severity: Severity::High,
                dimension: DqDimension::Accuracy,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("initial_margin_collected".into()),
                value: r.initial_margin_collected.map(|d| d.to_string()),
                message: "Initial margin collected is negative.".into(),
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
    fn flags_negative_im_collected() {
        let r = EmirRecord {
            initial_margin_collected: Some(Decimal::from(-5)),
            ..Default::default()
        };
        assert_eq!(
            NegativeInitialMarginCollected
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_positive() {
        let r = EmirRecord {
            initial_margin_collected: Some(Decimal::from(1)),
            ..Default::default()
        };
        assert!(NegativeInitialMarginCollected
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
