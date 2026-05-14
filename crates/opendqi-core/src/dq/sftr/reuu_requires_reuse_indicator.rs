//! SFTR.CON.REUU_REQUIRES_REUSE_INDICATOR — a reuse-update action
//! must carry the collateral reuse indicator.

use super::SftrCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};

/// Check implementation.
pub struct SftrReuuRequiresReuseIndicator;

const CHECK_ID: &str = "SFTR.CON.REUU_REQUIRES_REUSE_INDICATOR";

impl SftrCheck for SftrReuuRequiresReuseIndicator {
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
            .filter(|r| {
                r.action_type
                    .as_deref()
                    .map(|s| s.eq_ignore_ascii_case("REUU"))
                    .unwrap_or(false)
                    && r.reuse_indicator.is_none()
            })
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Sftr,
                severity: Severity::High,
                dimension: DqDimension::Consistency,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("reuse_indicator".into()),
                value: None,
                message: "Action type is REUU but no reuse indicator is reported.".into(),
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
    fn flags_reuu_without_indicator() {
        let r = SftrRecord {
            action_type: Some("REUU".into()),
            reuse_indicator: None,
            ..Default::default()
        };
        assert_eq!(
            SftrReuuRequiresReuseIndicator
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_reuu_with_indicator() {
        let r = SftrRecord {
            action_type: Some("REUU".into()),
            reuse_indicator: Some(true),
            ..Default::default()
        };
        assert!(SftrReuuRequiresReuseIndicator
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
