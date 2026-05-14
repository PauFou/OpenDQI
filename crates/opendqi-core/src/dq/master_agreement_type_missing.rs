//! EMIR.COMP.MASTER_AGREEMENT_TYPE_MISSING — master agreement type is
//! mandatory for most trades.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct MasterAgreementTypeMissing;

const CHECK_ID: &str = "EMIR.COMP.MASTER_AGREEMENT_TYPE_MISSING";

impl Check for MasterAgreementTypeMissing {
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
                r.master_agreement_type
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
                field: Some("master_agreement_type".into()),
                value: None,
                message: "Master agreement type is missing.".into(),
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
    fn flags_missing() {
        let r = EmirRecord::default();
        assert_eq!(
            MasterAgreementTypeMissing
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_present() {
        let r = EmirRecord {
            master_agreement_type: Some("ISDA".into()),
            ..Default::default()
        };
        assert!(MasterAgreementTypeMissing
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
