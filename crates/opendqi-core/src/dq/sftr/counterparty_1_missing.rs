//! SFTR.COMP.COUNTERPARTY_1_MISSING — reporting counterparty LEI required.

use super::SftrCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};

/// Check implementation.
pub struct SftrCounterparty1Missing;

const CHECK_ID: &str = "SFTR.COMP.COUNTERPARTY_1_MISSING";

impl SftrCheck for SftrCounterparty1Missing {
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
                r.counterparty_1
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
                field: Some("counterparty_1".into()),
                value: None,
                message: "Reporting counterparty LEI is missing.".into(),
                source_file: r.source_file.clone(),
                evidence: Vec::new(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_missing_rc() {
        let records = vec![
            SftrRecord {
                counterparty_1: Some("ABCDEFGHIJKLMNOPQR01".into()),
                ..Default::default()
            },
            SftrRecord {
                counterparty_1: None,
                ..Default::default()
            },
        ];
        let issues = SftrCounterparty1Missing.run(&records, &CheckContext::now_with_defaults());
        assert_eq!(issues.len(), 1);
    }
}
