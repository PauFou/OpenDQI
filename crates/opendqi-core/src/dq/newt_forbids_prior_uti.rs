//! EMIR.CON.NEWT_FORBIDS_PRIOR_UTI — a new trade should not carry a
//! prior UTI.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct NewtForbidsPriorUti;

const CHECK_ID: &str = "EMIR.CON.NEWT_FORBIDS_PRIOR_UTI";

impl Check for NewtForbidsPriorUti {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| {
                r.action_type.as_deref().map(|s| s.eq_ignore_ascii_case("NEWT")).unwrap_or(false)
                    && !r.prior_uti.as_deref().map(str::trim).unwrap_or("").is_empty()
            })
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Emir,
                severity: Severity::Warning,
                dimension: DqDimension::Consistency,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("prior_uti".into()),
                value: r.prior_uti.clone(),
                message: "Action type is NEWT but a prior UTI is reported — re-identification belongs to MODI.".into(),
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
    fn flags_newt_with_prior() {
        let r = EmirRecord {
            action_type: Some("NEWT".into()),
            prior_uti: Some("OLD".into()),
            ..Default::default()
        };
        assert_eq!(
            NewtForbidsPriorUti
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_newt_without_prior() {
        let r = EmirRecord {
            action_type: Some("NEWT".into()),
            prior_uti: None,
            ..Default::default()
        };
        assert!(NewtForbidsPriorUti
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
