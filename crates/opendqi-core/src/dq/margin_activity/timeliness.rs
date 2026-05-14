//! EMIR.MAR.TIMELINESS — `reporting_timestamp` is more than one
//! business day after `event_timestamp`.

use chrono::Duration;

use super::MarginActivityCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, MarginActivityRecord, Regime, Severity};

/// Check implementation.
pub struct EmirMarTimeliness;

const CHECK_ID: &str = "EMIR.MAR.TIMELINESS";

impl MarginActivityCheck for EmirMarTimeliness {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Timeliness
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(
        &self,
        records: &[MarginActivityRecord],
        _prior: &[MarginActivityRecord],
        ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        let max = Duration::hours(ctx.thresholds.timeliness.max_reporting_delay_hours);
        let mut out = Vec::new();
        for r in records {
            if let (Some(ev), Some(rep)) = (r.event_timestamp, r.reporting_timestamp) {
                if rep > ev && rep - ev > max {
                    let delay = rep - ev;
                    out.push(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Emir,
                        severity: Severity::High,
                        dimension: DqDimension::Timeliness,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("reporting_timestamp".into()),
                        value: Some(rep.to_rfc3339()),
                        message: format!(
                            "Reporting delay {}h exceeds threshold {}h.",
                            delay.num_hours(),
                            ctx.thresholds.timeliness.max_reporting_delay_hours
                        ),
                        source_file: r.source_file.clone(),
                        evidence: Vec::new(),
                    });
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Utc};

    fn ctx() -> CheckContext {
        CheckContext {
            thresholds: Default::default(),
            today: chrono::NaiveDate::from_ymd_opt(2026, 5, 13).unwrap(),
            now: chrono::DateTime::parse_from_rfc3339("2026-05-13T08:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
        }
    }

    fn ts(s: &str) -> Option<DateTime<Utc>> {
        Some(
            chrono::DateTime::parse_from_rfc3339(s)
                .unwrap()
                .with_timezone(&Utc),
        )
    }

    #[test]
    fn flags_late_reporting() {
        let r = MarginActivityRecord {
            event_timestamp: ts("2026-05-10T08:00:00Z"),
            reporting_timestamp: ts("2026-05-13T08:00:00Z"),
            ..Default::default()
        };
        let out = EmirMarTimeliness.run(&[r], &[], &ctx());
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn accepts_on_time() {
        let r = MarginActivityRecord {
            event_timestamp: ts("2026-05-13T07:00:00Z"),
            reporting_timestamp: ts("2026-05-13T08:00:00Z"),
            ..Default::default()
        };
        let out = EmirMarTimeliness.run(&[r], &[], &ctx());
        assert!(out.is_empty());
    }
}
