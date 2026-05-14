//! EMIR.COMP.NATURE_MISSING — nature of the reporting counterparty is
//! mandatory.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct NatureMissing;

const CHECK_ID: &str = "EMIR.COMP.NATURE_MISSING";

impl Check for NatureMissing {
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
            .filter(|r| r.nature.as_deref().map(str::trim).unwrap_or("").is_empty())
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Emir,
                severity: Severity::Warning,
                dimension: DqDimension::Completeness,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("nature".into()),
                value: None,
                message: "Nature of the reporting counterparty is missing.".into(),
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
    fn flags_missing_nature() {
        let r = EmirRecord::default();
        assert_eq!(
            NatureMissing
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_present_nature() {
        let r = EmirRecord {
            nature: Some("F".into()),
            ..Default::default()
        };
        assert!(NatureMissing
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
