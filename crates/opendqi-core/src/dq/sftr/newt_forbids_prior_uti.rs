//! SFTR.CON.NEWT_FORBIDS_PRIOR_UTI — a new SFT should not carry a prior UTI.

use super::SftrCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};

/// Check implementation.
pub struct SftrNewtForbidsPriorUti;

const CHECK_ID: &str = "SFTR.CON.NEWT_FORBIDS_PRIOR_UTI";

impl SftrCheck for SftrNewtForbidsPriorUti {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[SftrRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| {
                r.action_type
                    .as_deref()
                    .map(|s| s.eq_ignore_ascii_case("NEWT"))
                    .unwrap_or(false)
                    && r.prior_uti
                        .as_deref()
                        .map(|s| !s.trim().is_empty())
                        .unwrap_or(false)
            })
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Sftr,
                severity: Severity::Warning,
                dimension: DqDimension::Consistency,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("prior_uti".into()),
                value: r.prior_uti.clone(),
                message: "Action type is NEWT but a prior UTI is reported.".into(),
                source_file: r.source_file.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flags_newt_with_prior_uti() {
        let r = SftrRecord {
            action_type: Some("NEWT".into()),
            prior_uti: Some("OLD-UTI-001".into()),
            ..Default::default()
        };
        assert_eq!(
            SftrNewtForbidsPriorUti
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_modi() {
        let r = SftrRecord {
            action_type: Some("MODI".into()),
            prior_uti: Some("OLD-UTI-001".into()),
            ..Default::default()
        };
        assert!(SftrNewtForbidsPriorUti
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
