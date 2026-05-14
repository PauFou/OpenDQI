//! EMIR.CON.ETRM_REQUIRES_TERMINATION_DATE — an early-termination
//! report must carry a termination date.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct EtrmRequiresTerminationDate;

const CHECK_ID: &str = "EMIR.CON.ETRM_REQUIRES_TERMINATION_DATE";

impl Check for EtrmRequiresTerminationDate {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
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
                regime: Regime::Emir,
                severity: Severity::High,
                dimension: DqDimension::Consistency,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("termination_date".into()),
                value: None,
                message: "Action type is ETRM but no termination date is reported.".into(),
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
    fn flags_etrm_without_date() {
        let r = EmirRecord {
            action_type: Some("ETRM".into()),
            termination_date: None,
            ..Default::default()
        };
        assert_eq!(
            EtrmRequiresTerminationDate
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_etrm_with_date() {
        let r = EmirRecord {
            action_type: Some("ETRM".into()),
            termination_date: NaiveDate::from_ymd_opt(2026, 4, 1),
            ..Default::default()
        };
        assert!(EtrmRequiresTerminationDate
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
    #[test]
    fn ignores_non_etrm() {
        let r = EmirRecord {
            action_type: Some("NEWT".into()),
            termination_date: None,
            ..Default::default()
        };
        assert!(EtrmRequiresTerminationDate
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
