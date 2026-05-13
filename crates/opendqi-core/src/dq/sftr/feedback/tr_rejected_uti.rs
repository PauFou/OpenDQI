//! SFTR.FBK.TR_REJECTED_UTI — the TR rejected an SFT submission.

use super::SftrFeedbackCheck;
use crate::dq::CheckContext;
use crate::model::{
    DqDimension, DqIssue, FeedbackRecord, FeedbackType, Regime, Severity, SftrRecord,
};

/// Check implementation.
pub struct SftrTrRejectedUti;

const CHECK_ID: &str = "SFTR.FBK.TR_REJECTED_UTI";

impl SftrFeedbackCheck for SftrTrRejectedUti {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Validity
    }
    fn severity(&self) -> Severity {
        Severity::Critical
    }
    fn run(
        &self,
        feedback: &[FeedbackRecord],
        _prior: &[SftrRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        feedback
            .iter()
            .filter(|f| {
                matches!(f.regime, Regime::Sftr) && f.feedback_type == FeedbackType::Rejected
            })
            .map(|f| {
                let reason = f
                    .reason_code
                    .as_deref()
                    .map(|c| format!("[{c}] "))
                    .unwrap_or_default();
                let desc = f
                    .reason_description
                    .as_deref()
                    .unwrap_or("(no description)");
                let uti = f.uti.as_deref().unwrap_or("(unknown UTI)");
                DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Sftr,
                    severity: Severity::Critical,
                    dimension: DqDimension::Validity,
                    record_id: f.record_id.clone(),
                    uti: f.uti.clone(),
                    field: None,
                    value: f.reason_code.clone(),
                    message: format!("TR rejected report for UTI {uti}: {reason}{desc}"),
                    source_file: f.source_file.clone(),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_rejected() {
        let feedback = vec![FeedbackRecord {
            regime: Regime::Sftr,
            feedback_type: FeedbackType::Rejected,
            uti: Some("U1".into()),
            reason_code: Some("VAL01".into()),
            ..Default::default()
        }];
        let issues = SftrTrRejectedUti.run(&feedback, &[], &CheckContext::now_with_defaults());
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn ignores_other_regime() {
        let feedback = vec![FeedbackRecord {
            regime: Regime::Emir,
            feedback_type: FeedbackType::Rejected,
            ..Default::default()
        }];
        assert!(SftrTrRejectedUti
            .run(&feedback, &[], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
