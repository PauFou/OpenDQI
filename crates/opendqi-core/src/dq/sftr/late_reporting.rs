//! SFTR.TIM.LATE_REPORTING — reporting must follow the event within
//! the configured deadline.

use chrono::Duration;

use super::SftrCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};

/// Check implementation.
pub struct SftrLateReporting;

const CHECK_ID: &str = "SFTR.TIM.LATE_REPORTING";

impl SftrCheck for SftrLateReporting {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Timeliness
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[SftrRecord], ctx: &CheckContext) -> Vec<DqIssue> {
        let limit = Duration::hours(ctx.thresholds.timeliness.max_reporting_delay_hours);
        records
            .iter()
            .filter_map(|r| {
                let event = r.event_timestamp?;
                let report = r.reporting_timestamp?;
                let delta = report - event;
                if delta > limit {
                    let delta_hours = delta.num_minutes() as f64 / 60.0;
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Sftr,
                        severity: Severity::High,
                        dimension: DqDimension::Timeliness,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("reporting_timestamp".into()),
                        value: Some(report.to_rfc3339()),
                        message: format!(
                            "Reporting delay of {delta_hours:.1}h exceeds the {limit_h}h limit.",
                            limit_h = ctx.thresholds.timeliness.max_reporting_delay_hours
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
    fn flags_late_sftr_report() {
        let event = chrono::Utc.with_ymd_and_hms(2026, 5, 10, 8, 0, 0).unwrap();
        let report = chrono::Utc.with_ymd_and_hms(2026, 5, 12, 9, 0, 0).unwrap();
        let r = SftrRecord {
            uti: Some("A".into()),
            event_timestamp: Some(event),
            reporting_timestamp: Some(report),
            ..Default::default()
        };
        let ctx = CheckContext::now_with_defaults();
        let issues = SftrLateReporting.run(&[r], &ctx);
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn missing_timestamps_are_skipped() {
        let r = SftrRecord {
            uti: Some("A".into()),
            ..Default::default()
        };
        let issues = SftrLateReporting.run(&[r], &CheckContext::now_with_defaults());
        assert!(issues.is_empty());
    }
}
