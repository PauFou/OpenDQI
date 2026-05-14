//! EMIR.CON.REPORTING_BEFORE_EXECUTION — a report cannot be submitted
//! before the trade was executed (EMIR-VR-1001-04).

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct ReportingBeforeExecution;

const CHECK_ID: &str = "EMIR.CON.REPORTING_BEFORE_EXECUTION";

impl Check for ReportingBeforeExecution {
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
                let exec = r.execution_timestamp?;
                let report = r.reporting_timestamp?;
                if report < exec {
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Emir,
                        severity: Severity::High,
                        dimension: DqDimension::Consistency,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("reporting_timestamp".into()),
                        value: Some(report.to_rfc3339()),
                        message: format!(
                            "Reporting timestamp {} precedes execution timestamp {} (EMIR-VR-1001-04).",
                            report.to_rfc3339(),
                            exec.to_rfc3339()
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
    use chrono::TimeZone;

    #[test]
    fn flags_when_report_precedes_execution() {
        let ts = |d, h| chrono::Utc.with_ymd_and_hms(2026, 5, d, h, 0, 0).unwrap();
        let records = vec![
            EmirRecord {
                execution_timestamp: Some(ts(10, 9)),
                reporting_timestamp: Some(ts(10, 18)),
                ..Default::default()
            },
            EmirRecord {
                execution_timestamp: Some(ts(10, 9)),
                reporting_timestamp: Some(ts(10, 8)),
                ..Default::default()
            },
        ];
        let ctx = CheckContext::now_with_defaults();
        let issues = ReportingBeforeExecution.run(&records, &ctx);
        assert_eq!(issues.len(), 1);
    }
}
