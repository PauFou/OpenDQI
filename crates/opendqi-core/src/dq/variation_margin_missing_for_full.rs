//! EMIR.COMP.VARIATION_MARGIN_MISSING_FOR_FULL — fully-collateralised
//! trades should report a variation margin posted.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct VariationMarginMissingForFull;

const CHECK_ID: &str = "EMIR.COMP.VARIATION_MARGIN_MISSING_FOR_FULL";

impl Check for VariationMarginMissingForFull {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| {
                r.collateralisation_category
                    .as_deref()
                    .map(|s| s.eq_ignore_ascii_case("FLCL"))
                    .unwrap_or(false)
                    && r.variation_margin_posted.is_none()
            })
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Emir,
                severity: Severity::Warning,
                dimension: DqDimension::Completeness,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("variation_margin_posted".into()),
                value: None,
                message:
                    "Collateralisation category is FLCL but variation margin posted is missing."
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
    fn flags_flcl_without_vm() {
        let r = EmirRecord {
            collateralisation_category: Some("FLCL".into()),
            variation_margin_posted: None,
            ..Default::default()
        };
        assert_eq!(
            VariationMarginMissingForFull
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_with_vm() {
        let r = EmirRecord {
            collateralisation_category: Some("FLCL".into()),
            variation_margin_posted: Some(Decimal::from(1)),
            ..Default::default()
        };
        assert!(VariationMarginMissingForFull
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
