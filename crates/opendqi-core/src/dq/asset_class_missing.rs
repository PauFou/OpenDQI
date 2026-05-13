//! EMIR.COMP.ASSET_CLASS_MISSING — asset class is mandatory.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct AssetClassMissing;

const CHECK_ID: &str = "EMIR.COMP.ASSET_CLASS_MISSING";

impl Check for AssetClassMissing {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| {
                r.asset_class
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or("")
                    .is_empty()
            })
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Emir,
                severity: Severity::High,
                dimension: DqDimension::Completeness,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("asset_class".into()),
                value: None,
                message: "Asset class is missing.".into(),
                source_file: r.source_file.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flags_missing() {
        let r = EmirRecord::default();
        assert_eq!(
            AssetClassMissing
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_present() {
        let r = EmirRecord {
            asset_class: Some("IR".into()),
            ..Default::default()
        };
        assert!(AssetClassMissing
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
