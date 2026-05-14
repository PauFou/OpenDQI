//! EMIR.CON.MATURITY_IN_PAST — an outstanding trade should not have
//! a maturity date already in the past.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct MaturityInPast;

const CHECK_ID: &str = "EMIR.CON.MATURITY_IN_PAST";

impl Check for MaturityInPast {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[EmirRecord], ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter_map(|r| {
                if !r.is_outstanding(ctx.today) {
                    return None;
                }
                let m = r.maturity_date?;
                if m < ctx.today {
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Emir,
                        severity: Severity::Warning,
                        dimension: DqDimension::Consistency,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("maturity_date".into()),
                        value: Some(m.to_string()),
                        message: format!("Outstanding trade has maturity {m} already in the past (today={today}).", today = ctx.today),
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

    fn ctx() -> CheckContext {
        let mut c = CheckContext::now_with_defaults();
        c.today = NaiveDate::from_ymd_opt(2026, 5, 13).unwrap();
        c
    }
    #[test]
    fn flags_past_maturity() {
        let r = EmirRecord {
            maturity_date: NaiveDate::from_ymd_opt(2026, 4, 1),
            termination_date: None,
            ..Default::default()
        };
        assert_eq!(MaturityInPast.run(&[r], &ctx()).len(), 1);
    }
    #[test]
    fn ignores_future_maturity() {
        let r = EmirRecord {
            maturity_date: NaiveDate::from_ymd_opt(2030, 1, 1),
            ..Default::default()
        };
        assert!(MaturityInPast.run(&[r], &ctx()).is_empty());
    }
}
