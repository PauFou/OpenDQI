//! EMIR.COMP.MASTER_AGREEMENT_VERSION_MISSING — once a master
//! agreement type is reported, a version is expected too.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct MasterAgreementVersionMissing;

const CHECK_ID: &str = "EMIR.COMP.MASTER_AGREEMENT_VERSION_MISSING";

impl Check for MasterAgreementVersionMissing {
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
                !r.master_agreement_type
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or("")
                    .is_empty()
                    && r.master_agreement_version
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
                field: Some("master_agreement_version".into()),
                value: None,
                message: "Master agreement type is set but the version is missing.".into(),
                source_file: r.source_file.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flags_missing_version() {
        let r = EmirRecord {
            master_agreement_type: Some("ISDA".into()),
            master_agreement_version: None,
            ..Default::default()
        };
        assert_eq!(
            MasterAgreementVersionMissing
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_with_version() {
        let r = EmirRecord {
            master_agreement_type: Some("ISDA".into()),
            master_agreement_version: Some("2002".into()),
            ..Default::default()
        };
        assert!(MasterAgreementVersionMissing
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
