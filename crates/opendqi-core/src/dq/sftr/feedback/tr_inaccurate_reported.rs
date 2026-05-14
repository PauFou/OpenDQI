//! SFTR.FBK.TR_INACCURATE_REPORTED — TR accepted the report but
//! flagged inaccurate field(s).

use super::SftrFeedbackCheck;
use crate::dq::CheckContext;
use crate::model::{
    DqDimension, DqIssue, FeedbackRecord, FeedbackType, Regime, Severity, SftrRecord,
};

/// Check implementation.
pub struct SftrTrInaccurateReported;

const CHECK_ID: &str = "SFTR.FBK.TR_INACCURATE_REPORTED";

impl SftrFeedbackCheck for SftrTrInaccurateReported {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
    }
    fn severity(&self) -> Severity {
        Severity::High
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
                matches!(f.regime, Regime::Sftr) && f.feedback_type == FeedbackType::Inaccurate
            })
            .map(|f| {
                let uti = f.uti.as_deref().unwrap_or("(unknown UTI)");
                let field = f.reported_field.as_deref().unwrap_or("(unspecified field)");
                let desc = f
                    .reason_description
                    .as_deref()
                    .unwrap_or("(no description)");
                DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Sftr,
                    severity: Severity::High,
                    dimension: DqDimension::Accuracy,
                    record_id: f.record_id.clone(),
                    uti: f.uti.clone(),
                    field: f.reported_field.clone(),
                    value: None,
                    message: format!(
                        "TR flagged UTI {uti} as inaccurate on field '{field}': {desc}"
                    ),
                    source_file: f.source_file.clone(),
                    evidence: Vec::new(),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_inaccurate() {
        let feedback = vec![FeedbackRecord {
            regime: Regime::Sftr,
            feedback_type: FeedbackType::Inaccurate,
            uti: Some("U1".into()),
            reported_field: Some("LoanValue".into()),
            ..Default::default()
        }];
        assert_eq!(
            SftrTrInaccurateReported
                .run(&feedback, &[], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }

    #[test]
    fn ignores_emir_regime() {
        let feedback = vec![FeedbackRecord {
            regime: Regime::Emir,
            feedback_type: FeedbackType::Inaccurate,
            ..Default::default()
        }];
        assert!(SftrTrInaccurateReported
            .run(&feedback, &[], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
