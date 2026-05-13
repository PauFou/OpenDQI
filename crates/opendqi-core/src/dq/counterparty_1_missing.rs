//! EMIR.COMP.COUNTERPARTY_1_MISSING — the reporting counterparty LEI
//! is mandatory under EMIR-VR-1003.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct Counterparty1Missing;

const CHECK_ID: &str = "EMIR.COMP.COUNTERPARTY_1_MISSING";

impl Check for Counterparty1Missing {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
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
                regime: Regime::Emir,
                severity: Severity::High,
                dimension: DqDimension::Completeness,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("counterparty_1".into()),
                value: None,
                message: "Reporting counterparty LEI is missing (EMIR-VR-1003).".into(),
                source_file: r.source_file.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_records_without_counterparty_1() {
        let records = vec![
            EmirRecord {
                counterparty_1: Some("DUMMYCPTY10000000001".into()),
                ..Default::default()
            },
            EmirRecord {
                counterparty_1: None,
                ..Default::default()
            },
            EmirRecord {
                counterparty_1: Some("  ".into()),
                ..Default::default()
            },
        ];
        let ctx = CheckContext::now_with_defaults();
        let issues = Counterparty1Missing.run(&records, &ctx);
        assert_eq!(issues.len(), 2);
    }
}
