//! SFTR.CON.NEWT_FORBIDS_TERMINATION_DATE — a new SFT should not
//! report a termination date.

use super::SftrCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};

/// Check implementation.
pub struct SftrNewtForbidsTerminationDate;

const CHECK_ID: &str = "SFTR.CON.NEWT_FORBIDS_TERMINATION_DATE";

impl SftrCheck for SftrNewtForbidsTerminationDate {
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
                    && r.termination_date.is_some()
            })
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Sftr,
                severity: Severity::Warning,
                dimension: DqDimension::Consistency,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("termination_date".into()),
                value: r.termination_date.map(|d| d.to_string()),
                message: "Action type is NEWT but a termination date is reported.".into(),
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
    fn flags_newt_with_termination() {
        let r = SftrRecord {
            action_type: Some("NEWT".into()),
            termination_date: NaiveDate::from_ymd_opt(2026, 5, 1),
            ..Default::default()
        };
        assert_eq!(
            SftrNewtForbidsTerminationDate
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_etrm() {
        let r = SftrRecord {
            action_type: Some("ETRM".into()),
            termination_date: NaiveDate::from_ymd_opt(2026, 5, 1),
            ..Default::default()
        };
        assert!(SftrNewtForbidsTerminationDate
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
