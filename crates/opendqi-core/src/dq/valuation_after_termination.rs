//! EMIR.CON.VALUATION_AFTER_TERMINATION — once a trade is terminated
//! it should not receive further valuations.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct ValuationAfterTermination;

const CHECK_ID: &str = "EMIR.CON.VALUATION_AFTER_TERMINATION";

impl Check for ValuationAfterTermination {
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
                let term = r.termination_date?;
                let val = r.valuation_timestamp?;
                if val.date_naive() > term {
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Emir,
                        severity: Severity::High,
                        dimension: DqDimension::Consistency,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("valuation_timestamp".into()),
                        value: Some(val.to_rfc3339()),
                        message: format!(
                            "Valuation observed on {} after the termination date {}.",
                            val.date_naive(),
                            term
                        ),
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
    use chrono::{NaiveDate, TimeZone};

    #[test]
    fn flags_when_valuation_is_after_termination() {
        let val = chrono::Utc.with_ymd_and_hms(2026, 5, 15, 12, 0, 0).unwrap();
        let term_before = NaiveDate::from_ymd_opt(2026, 5, 10).unwrap();
        let term_after = NaiveDate::from_ymd_opt(2026, 5, 20).unwrap();
        let records = vec![
            EmirRecord {
                termination_date: Some(term_before),
                valuation_timestamp: Some(val),
                ..Default::default()
            },
            EmirRecord {
                termination_date: Some(term_after),
                valuation_timestamp: Some(val),
                ..Default::default()
            },
            EmirRecord {
                termination_date: None,
                valuation_timestamp: Some(val),
                ..Default::default()
            },
        ];
        let ctx = CheckContext::now_with_defaults();
        let issues = ValuationAfterTermination.run(&records, &ctx);
        assert_eq!(issues.len(), 1);
    }
}
