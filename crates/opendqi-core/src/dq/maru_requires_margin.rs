//! EMIR.CON.MARU_REQUIRES_MARGIN — a margin-update report must carry at
//! least one margin amount.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct MaruRequiresMargin;

const CHECK_ID: &str = "EMIR.CON.MARU_REQUIRES_MARGIN";

impl Check for MaruRequiresMargin {
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
            .filter(|r| {
                if r.action_type.as_deref().unwrap_or("") != "MARU" {
                    return false;
                }
                r.initial_margin_posted.is_none()
                    && r.initial_margin_collected.is_none()
                    && r.variation_margin_posted.is_none()
                    && r.variation_margin_collected.is_none()
            })
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Emir,
                severity: Severity::High,
                dimension: DqDimension::Consistency,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("action_type".into()),
                value: Some("MARU".into()),
                message: "Action type is MARU but no margin amount is reported.".into(),
                source_file: r.source_file.clone(),
                evidence: Vec::new(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    #[test]
    fn flags_empty_maru() {
        let r = EmirRecord {
            action_type: Some("MARU".into()),
            ..Default::default()
        };
        assert_eq!(
            MaruRequiresMargin
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_maru_with_margin() {
        let r = EmirRecord {
            action_type: Some("MARU".into()),
            initial_margin_posted: Some(Decimal::from(100)),
            ..Default::default()
        };
        assert!(MaruRequiresMargin
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
    #[test]
    fn ignores_non_maru() {
        let r = EmirRecord {
            action_type: Some("NEWT".into()),
            ..Default::default()
        };
        assert!(MaruRequiresMargin
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
