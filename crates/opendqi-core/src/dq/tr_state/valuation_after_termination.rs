//! EMIR.TST.VALUATION_AFTER_TERMINATION — TSR row carries both a
//! termination date and a valuation timestamp that is strictly
//! after termination.

use crate::dq::{CheckContext, TrStateCheck};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity, TrStateRecord};

/// Check implementation.
pub struct EmirTsrValuationAfterTermination;

const CHECK_ID: &str = "EMIR.TST.VALUATION_AFTER_TERMINATION";

impl TrStateCheck for EmirTsrValuationAfterTermination {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(
        &self,
        records: &[TrStateRecord],
        _prior: &[EmirRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        records
            .iter()
            .filter_map(|r| {
                let term = r.termination_date?;
                let val_ts = r.valuation_timestamp?;
                if val_ts.date_naive() <= term {
                    return None;
                }
                Some(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Emir,
                    severity: Severity::High,
                    dimension: DqDimension::Consistency,
                    record_id: r.record_id.clone(),
                    uti: r.uti.clone(),
                    field: Some("valuation_timestamp".into()),
                    value: Some(val_ts.to_rfc3339()),
                    message: format!(
                        "TR holds a valuation at {ts} but the termination date is {term} — valuation post-dates termination.",
                        ts = val_ts.to_rfc3339(),
                    ),
                    source_file: r.source_file.clone(),
                    evidence: Vec::new(),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, TimeZone, Utc};

    #[test]
    fn flags_post_termination_valuation() {
        let rec = TrStateRecord {
            uti: Some("U1".into()),
            termination_date: NaiveDate::from_ymd_opt(2026, 4, 1),
            valuation_timestamp: Some(Utc.with_ymd_and_hms(2026, 4, 15, 0, 0, 0).unwrap()),
            ..Default::default()
        };
        assert_eq!(
            EmirTsrValuationAfterTermination
                .run(&[rec], &[], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }

    #[test]
    fn ignores_valuation_before_termination() {
        let rec = TrStateRecord {
            uti: Some("U1".into()),
            termination_date: NaiveDate::from_ymd_opt(2026, 4, 15),
            valuation_timestamp: Some(Utc.with_ymd_and_hms(2026, 4, 1, 0, 0, 0).unwrap()),
            ..Default::default()
        };
        assert!(EmirTsrValuationAfterTermination
            .run(&[rec], &[], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
