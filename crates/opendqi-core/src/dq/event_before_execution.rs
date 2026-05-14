//! EMIR.CON.EVENT_BEFORE_EXECUTION — event must not precede execution.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct EventBeforeExecution;

const CHECK_ID: &str = "EMIR.CON.EVENT_BEFORE_EXECUTION";

impl Check for EventBeforeExecution {
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
                let event = r.event_timestamp?;
                if event < exec {
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Emir,
                        severity: Severity::High,
                        dimension: DqDimension::Consistency,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("event_timestamp".into()),
                        value: Some(event.to_rfc3339()),
                        message: format!(
                            "Event timestamp {} precedes execution timestamp {}.",
                            event.to_rfc3339(),
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
    fn flags_event_before_exec() {
        let exec = chrono::Utc.with_ymd_and_hms(2026, 4, 10, 9, 0, 0).unwrap();
        let event = chrono::Utc.with_ymd_and_hms(2026, 4, 9, 9, 0, 0).unwrap();
        let r = EmirRecord {
            execution_timestamp: Some(exec),
            event_timestamp: Some(event),
            ..Default::default()
        };
        assert_eq!(
            EventBeforeExecution
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_event_after_exec() {
        let exec = chrono::Utc.with_ymd_and_hms(2026, 4, 10, 9, 0, 0).unwrap();
        let event = chrono::Utc.with_ymd_and_hms(2026, 4, 10, 10, 0, 0).unwrap();
        let r = EmirRecord {
            execution_timestamp: Some(exec),
            event_timestamp: Some(event),
            ..Default::default()
        };
        assert!(EventBeforeExecution
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
