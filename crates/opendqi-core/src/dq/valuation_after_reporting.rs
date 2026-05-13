//! EMIR.TIM.VALUATION_AFTER_REPORTING — a valuation cannot be
//! observed after the report itself was submitted (EMIR-VR-1001-05).

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct ValuationAfterReporting;

const CHECK_ID: &str = "EMIR.TIM.VALUATION_AFTER_REPORTING";

impl Check for ValuationAfterReporting {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Timeliness
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter_map(|r| {
                let val = r.valuation_timestamp?;
                let report = r.reporting_timestamp?;
                if val > report {
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Emir,
                        severity: Severity::Warning,
                        dimension: DqDimension::Timeliness,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("valuation_timestamp".into()),
                        value: Some(val.to_rfc3339()),
                        message: format!(
                            "Valuation timestamp {} is after the reporting timestamp {}.",
                            val.to_rfc3339(),
                            report.to_rfc3339()
                        ),
                        source_file: r.source_file.clone(),
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
    use chrono::TimeZone;

    #[test]
    fn flags_when_valuation_after_report() {
        let ts = |d, h| chrono::Utc.with_ymd_and_hms(2026, 5, d, h, 0, 0).unwrap();
        let records = vec![
            EmirRecord {
                valuation_timestamp: Some(ts(10, 12)),
                reporting_timestamp: Some(ts(10, 18)),
                ..Default::default()
            },
            EmirRecord {
                valuation_timestamp: Some(ts(11, 12)),
                reporting_timestamp: Some(ts(10, 18)),
                ..Default::default()
            },
        ];
        let ctx = CheckContext::now_with_defaults();
        let issues = ValuationAfterReporting.run(&records, &ctx);
        assert_eq!(issues.len(), 1);
    }
}
