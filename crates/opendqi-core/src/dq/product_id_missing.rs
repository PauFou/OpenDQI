//! EMIR.COMP.PRODUCT_ID_MISSING — a product identifier is expected
//! whenever an asset class is declared.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct ProductIdMissing;

const CHECK_ID: &str = "EMIR.COMP.PRODUCT_ID_MISSING";

impl Check for ProductIdMissing {
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
                !r.asset_class
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or("")
                    .is_empty()
                    && r.product_id
                        .as_deref()
                        .map(str::trim)
                        .unwrap_or("")
                        .is_empty()
            })
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Emir,
                severity: Severity::Warning,
                dimension: DqDimension::Completeness,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("product_id".into()),
                value: None,
                message: "Product identifier is missing while an asset class is declared.".into(),
                source_file: r.source_file.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flags_when_missing() {
        let r = EmirRecord {
            asset_class: Some("IR".into()),
            product_id: None,
            ..Default::default()
        };
        assert_eq!(
            ProductIdMissing
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_when_present() {
        let r = EmirRecord {
            asset_class: Some("IR".into()),
            product_id: Some("X".into()),
            ..Default::default()
        };
        assert!(ProductIdMissing
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
    #[test]
    fn ignores_when_asset_class_missing() {
        let r = EmirRecord {
            asset_class: None,
            product_id: None,
            ..Default::default()
        };
        assert!(ProductIdMissing
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
