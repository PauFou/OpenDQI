//! EMIR.COMP.COUNTERPARTY_2_MISSING — the other counterparty LEI is
//! mandatory under EMIR-VR-1006.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct Counterparty2Missing;

const CHECK_ID: &str = "EMIR.COMP.COUNTERPARTY_2_MISSING";

impl Check for Counterparty2Missing {
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
                r.counterparty_2
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
                field: Some("counterparty_2".into()),
                value: None,
                message: "Other counterparty LEI is missing (EMIR-VR-1006).".into(),
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
    fn flags_records_without_counterparty_2() {
        let records = vec![
            EmirRecord {
                counterparty_2: Some("DUMMYCPTY20000000001".into()),
                ..Default::default()
            },
            EmirRecord {
                counterparty_2: None,
                ..Default::default()
            },
        ];
        let ctx = CheckContext::now_with_defaults();
        let issues = Counterparty2Missing.run(&records, &ctx);
        assert_eq!(issues.len(), 1);
    }
}
