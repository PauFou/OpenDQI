//! EMIR.CON.NEWT_FORBIDS_TERMINATION_DATE — a new trade should not
//! report a termination date.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct NewtForbidsTerminationDate;

const CHECK_ID: &str = "EMIR.CON.NEWT_FORBIDS_TERMINATION_DATE";

impl Check for NewtForbidsTerminationDate {
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
                r.action_type
                    .as_deref()
                    .map(|s| s.eq_ignore_ascii_case("NEWT"))
                    .unwrap_or(false)
                    && r.termination_date.is_some()
            })
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Emir,
                severity: Severity::Warning,
                dimension: DqDimension::Consistency,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("termination_date".into()),
                value: r.termination_date.map(|d| d.to_string()),
                message: "Action type is NEWT but a termination date is reported.".into(),
                source_file: r.source_file.clone(),
                evidence: Vec::new(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    #[test]
    fn flags_newt_with_term() {
        let r = EmirRecord {
            action_type: Some("NEWT".into()),
            termination_date: NaiveDate::from_ymd_opt(2026, 4, 1),
            ..Default::default()
        };
        assert_eq!(
            NewtForbidsTerminationDate
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_newt_without_term() {
        let r = EmirRecord {
            action_type: Some("NEWT".into()),
            termination_date: None,
            ..Default::default()
        };
        assert!(NewtForbidsTerminationDate
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
