//! SFTR.COMP.COUNTERPARTY_2_MISSING — other counterparty LEI required.

use super::SftrCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};

/// Check implementation.
pub struct SftrCounterparty2Missing;

const CHECK_ID: &str = "SFTR.COMP.COUNTERPARTY_2_MISSING";

impl SftrCheck for SftrCounterparty2Missing {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[SftrRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| {
                r.counterparty_2
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or("")
                    .is_empty()
            })
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Sftr,
                severity: Severity::High,
                dimension: DqDimension::Completeness,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("counterparty_2".into()),
                value: None,
                message: "Other counterparty LEI is missing.".into(),
                source_file: r.source_file.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_missing_oc() {
        let records = vec![
            SftrRecord {
                counterparty_2: Some("ABCDEFGHIJKLMNOPQR02".into()),
                ..Default::default()
            },
            SftrRecord {
                counterparty_2: None,
                ..Default::default()
            },
        ];
        let issues = SftrCounterparty2Missing.run(&records, &CheckContext::now_with_defaults());
        assert_eq!(issues.len(), 1);
    }
}
