//! EMIR.CON.TERMINATION_AFTER_MATURITY — termination cannot fall
//! after the maturity date.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct TerminationAfterMaturity;

const CHECK_ID: &str = "EMIR.CON.TERMINATION_AFTER_MATURITY";

impl Check for TerminationAfterMaturity {
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
            .filter_map(|r| {
                let m = r.maturity_date?;
                let t = r.termination_date?;
                if t > m {
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Emir,
                        severity: Severity::High,
                        dimension: DqDimension::Consistency,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("termination_date".into()),
                        value: Some(t.to_string()),
                        message: format!("Termination date {t} is after maturity date {m}."),
                        source_file: r.source_file.clone(),
                        evidence: Vec::new(),
                    })
                } else {
                    None
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    #[test]
    fn flags_termination_after_maturity() {
        let r = EmirRecord {
            maturity_date: NaiveDate::from_ymd_opt(2026, 4, 1),
            termination_date: NaiveDate::from_ymd_opt(2026, 5, 1),
            ..Default::default()
        };
        assert_eq!(
            TerminationAfterMaturity
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_termination_before_maturity() {
        let r = EmirRecord {
            maturity_date: NaiveDate::from_ymd_opt(2026, 7, 1),
            termination_date: NaiveDate::from_ymd_opt(2026, 5, 1),
            ..Default::default()
        };
        assert!(TerminationAfterMaturity
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
