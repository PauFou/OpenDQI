//! EMIR.CON.EFFECTIVE_AFTER_MATURITY — effective date cannot fall
//! after the maturity date.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct EffectiveAfterMaturity;

const CHECK_ID: &str = "EMIR.CON.EFFECTIVE_AFTER_MATURITY";

impl Check for EffectiveAfterMaturity {
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
                let e = r.effective_date?;
                if e > m {
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Emir,
                        severity: Severity::High,
                        dimension: DqDimension::Consistency,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("effective_date".into()),
                        value: Some(e.to_string()),
                        message: format!("Effective date {e} is after maturity date {m}."),
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
    fn flags_effective_after_maturity() {
        let r = EmirRecord {
            effective_date: NaiveDate::from_ymd_opt(2026, 8, 1),
            maturity_date: NaiveDate::from_ymd_opt(2026, 4, 1),
            ..Default::default()
        };
        assert_eq!(
            EffectiveAfterMaturity
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_effective_before_maturity() {
        let r = EmirRecord {
            effective_date: NaiveDate::from_ymd_opt(2026, 1, 1),
            maturity_date: NaiveDate::from_ymd_opt(2026, 4, 1),
            ..Default::default()
        };
        assert!(EffectiveAfterMaturity
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
