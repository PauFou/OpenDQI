//! EMIR.ACC.NEGATIVE_VARIATION_MARGIN_POSTED — variation margin posted ≥ 0.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};
use rust_decimal::Decimal;

/// Check implementation.
pub struct NegativeVariationMarginPosted;

const CHECK_ID: &str = "EMIR.ACC.NEGATIVE_VARIATION_MARGIN_POSTED";

impl Check for NegativeVariationMarginPosted {
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
                r.variation_margin_posted
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
                field: Some("variation_margin_posted".into()),
                value: r.variation_margin_posted.map(|d| d.to_string()),
                message: "Variation margin posted is negative.".into(),
                source_file: r.source_file.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flags_negative_vm_posted() {
        let r = EmirRecord {
            variation_margin_posted: Some(Decimal::from(-1)),
            ..Default::default()
        };
        assert_eq!(
            NegativeVariationMarginPosted
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_positive() {
        let r = EmirRecord {
            variation_margin_posted: Some(Decimal::from(100)),
            ..Default::default()
        };
        assert!(NegativeVariationMarginPosted
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
