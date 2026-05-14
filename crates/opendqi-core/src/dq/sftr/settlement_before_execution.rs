//! SFTR.CON.SETTLEMENT_BEFORE_EXECUTION — settlement must follow execution.

use super::SftrCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};

/// Check implementation.
pub struct SftrSettlementBeforeExecution;

const CHECK_ID: &str = "SFTR.CON.SETTLEMENT_BEFORE_EXECUTION";

impl SftrCheck for SftrSettlementBeforeExecution {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[SftrRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter_map(|r| {
                let settlement = r.settlement_date?;
                let exec = r.execution_timestamp?;
                if settlement < exec.date_naive() {
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Sftr,
                        severity: Severity::High,
                        dimension: DqDimension::Consistency,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("settlement_date".into()),
                        value: Some(settlement.to_string()),
                        message: format!(
                            "Settlement date {settlement} precedes execution timestamp {}.",
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
    use chrono::{NaiveDate, TimeZone};

    #[test]
    fn flags_settlement_before_execution() {
        let exec = chrono::Utc.with_ymd_and_hms(2026, 4, 10, 9, 0, 0).unwrap();
        let records = vec![
            SftrRecord {
                execution_timestamp: Some(exec),
                settlement_date: NaiveDate::from_ymd_opt(2026, 4, 12),
                ..Default::default()
            },
            SftrRecord {
                execution_timestamp: Some(exec),
                settlement_date: NaiveDate::from_ymd_opt(2026, 4, 5),
                ..Default::default()
            },
        ];
        let issues =
            SftrSettlementBeforeExecution.run(&records, &CheckContext::now_with_defaults());
        assert_eq!(issues.len(), 1);
    }
}
