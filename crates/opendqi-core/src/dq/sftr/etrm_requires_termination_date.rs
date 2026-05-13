//! SFTR.CON.ETRM_REQUIRES_TERMINATION_DATE — an early-termination
//! action must carry a termination date.

use super::SftrCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};

/// Check implementation.
pub struct SftrEtrmRequiresTerminationDate;

const CHECK_ID: &str = "SFTR.CON.ETRM_REQUIRES_TERMINATION_DATE";

impl SftrCheck for SftrEtrmRequiresTerminationDate {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[SftrRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| {
                r.action_type
                    .as_deref()
                    .map(|s| s.eq_ignore_ascii_case("ETRM"))
                    .unwrap_or(false)
                    && r.termination_date.is_none()
            })
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Sftr,
                severity: Severity::High,
                dimension: DqDimension::Consistency,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("termination_date".into()),
                value: None,
                message: "Action type is ETRM but no termination date is reported.".into(),
                source_file: r.source_file.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    #[test]
    fn flags_etrm_without_termination() {
        let r = SftrRecord {
            action_type: Some("ETRM".into()),
            termination_date: None,
            ..Default::default()
        };
        assert_eq!(
            SftrEtrmRequiresTerminationDate
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_etrm_with_termination() {
        let r = SftrRecord {
            action_type: Some("ETRM".into()),
            termination_date: NaiveDate::from_ymd_opt(2026, 5, 1),
            ..Default::default()
        };
        assert!(SftrEtrmRequiresTerminationDate
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
