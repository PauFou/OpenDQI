//! EMIR.TIM.LATE_REPORTING — reporting must follow the event within the
//! configured deadline.

use chrono::Duration;

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct LateReporting;

const CHECK_ID: &str = "EMIR.TIM.LATE_REPORTING";

impl Check for LateReporting {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Timeliness
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[EmirRecord], ctx: &CheckContext) -> Vec<DqIssue> {
        let limit = Duration::hours(ctx.thresholds.timeliness.max_reporting_delay_hours);
        records
            .iter()
            .filter_map(|r| {
                let (event, report) = match (r.event_timestamp, r.reporting_timestamp) {
                    (Some(e), Some(r)) => (e, r),
                    _ => return None,
                };
                let delta = report - event;
                if delta > limit {
                    let delta_hours = delta.num_minutes() as f64 / 60.0;
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Emir,
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

    fn ctx() -> CheckContext {
        CheckContext::now_with_defaults()
    }

    #[test]
    fn flags_late_report() {
        let event = chrono::Utc.with_ymd_and_hms(2026, 5, 10, 8, 0, 0).unwrap();
        let report = chrono::Utc.with_ymd_and_hms(2026, 5, 12, 9, 0, 0).unwrap();
        let r = EmirRecord {
            uti: Some("A".into()),
            event_timestamp: Some(event),
            reporting_timestamp: Some(report),
            ..Default::default()
        };
        let issues = LateReporting.run(&[r], &ctx());
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn on_time_report_is_not_flagged() {
        let event = chrono::Utc.with_ymd_and_hms(2026, 5, 10, 8, 0, 0).unwrap();
        let report = chrono::Utc.with_ymd_and_hms(2026, 5, 11, 7, 59, 0).unwrap();
        let r = EmirRecord {
            uti: Some("A".into()),
            event_timestamp: Some(event),
            reporting_timestamp: Some(report),
            ..Default::default()
        };
        let issues = LateReporting.run(&[r], &ctx());
        assert!(issues.is_empty());
    }

    #[test]
    fn missing_timestamps_are_skipped() {
        let r = EmirRecord {
            uti: Some("A".into()),
            ..Default::default()
        };
        let issues = LateReporting.run(&[r], &ctx());
        assert!(issues.is_empty());
    }
}
