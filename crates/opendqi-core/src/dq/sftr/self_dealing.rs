//! SFTR.CON.SELF_DEALING — the two counterparties of an SFT must be distinct.

use super::SftrCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};

/// Check implementation.
pub struct SftrSelfDealing;

const CHECK_ID: &str = "SFTR.CON.SELF_DEALING";

impl SftrCheck for SftrSelfDealing {
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
                let c1 = r.counterparty_1.as_deref()?.trim();
                let c2 = r.counterparty_2.as_deref()?.trim();
                if c1.is_empty() || c2.is_empty() || !c1.eq_ignore_ascii_case(c2) {
                    None
                } else {
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Sftr,
                        severity: Severity::High,
                        dimension: DqDimension::Consistency,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("counterparty_2".into()),
                        value: Some(c2.to_owned()),
                        message: format!("Counterparties are identical ('{c1}'): self-dealing."),
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
    fn flags_self_dealing() {
        let r = SftrRecord {
            counterparty_1: Some("12345678901234567890".into()),
            counterparty_2: Some("12345678901234567890".into()),
            ..Default::default()
        };
        assert_eq!(
            SftrSelfDealing
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_distinct() {
        let r = SftrRecord {
            counterparty_1: Some("12345678901234567890".into()),
            counterparty_2: Some("09876543210987654321".into()),
            ..Default::default()
        };
        assert!(SftrSelfDealing
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
