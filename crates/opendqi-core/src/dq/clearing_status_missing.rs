//! EMIR.COMP.CLEARING_STATUS_MISSING — clearing status is mandatory.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct ClearingStatusMissing;

const CHECK_ID: &str = "EMIR.COMP.CLEARING_STATUS_MISSING";

impl Check for ClearingStatusMissing {
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
                r.clearing_status
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
                field: Some("clearing_status".into()),
                value: None,
                message: "Clearing status is missing.".into(),
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
            ClearingStatusMissing
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_present() {
        let r = EmirRecord {
            clearing_status: Some("CLRD".into()),
            ..Default::default()
        };
        assert!(ClearingStatusMissing
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
