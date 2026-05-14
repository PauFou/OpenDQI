//! EMIR.CON.SELF_DEALING — the two counterparties of a trade must
//! be distinct entities.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct SelfDealing;

const CHECK_ID: &str = "EMIR.CON.SELF_DEALING";

impl Check for SelfDealing {
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
                let a = r.counterparty_1.as_deref()?.trim();
                let b = r.counterparty_2.as_deref()?.trim();
                if a.is_empty() || b.is_empty() || !a.eq_ignore_ascii_case(b) {
                    None
                } else {
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Emir,
                        severity: Severity::High,
                        dimension: DqDimension::Consistency,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("counterparty_2".into()),
                        value: Some(b.to_owned()),
                        message: format!(
                            "Reporting counterparty and other counterparty are identical ({a})."
                        ),
                        source_file: r.source_file.clone(),
                        evidence: Vec::new(),
                    })
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flags_same_party() {
        let r = EmirRecord {
            counterparty_1: Some("LEI".into()),
            counterparty_2: Some("LEI".into()),
            ..Default::default()
        };
        assert_eq!(
            SelfDealing
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_different_parties() {
        let r = EmirRecord {
            counterparty_1: Some("LEI-A".into()),
            counterparty_2: Some("LEI-B".into()),
            ..Default::default()
        };
        assert!(SelfDealing
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
