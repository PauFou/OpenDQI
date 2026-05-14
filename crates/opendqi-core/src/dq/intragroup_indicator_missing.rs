//! EMIR.COMP.INTRAGROUP_INDICATOR_MISSING — the intragroup flag is
//! mandatory under EMIR Refit.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct IntragroupIndicatorMissing;

const CHECK_ID: &str = "EMIR.COMP.INTRAGROUP_INDICATOR_MISSING";

impl Check for IntragroupIndicatorMissing {
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
            .filter(|r| r.intragroup_indicator.is_none())
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Emir,
                severity: Severity::Warning,
                dimension: DqDimension::Completeness,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("intragroup_indicator".into()),
                value: None,
                message: "Intragroup transaction indicator is missing.".into(),
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
    fn flags_missing_intragroup() {
        let r = EmirRecord::default();
        assert_eq!(
            IntragroupIndicatorMissing
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_present_intragroup() {
        let r = EmirRecord {
            intragroup_indicator: Some(false),
            ..Default::default()
        };
        assert!(IntragroupIndicatorMissing
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
