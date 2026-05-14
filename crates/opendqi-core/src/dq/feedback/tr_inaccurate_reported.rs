//! EMIR.FBK.TR_INACCURATE_REPORTED — the TR accepted the report but
//! flagged inaccurate field(s).

use crate::dq::{CheckContext, FeedbackCheck};
use crate::model::{
    DqDimension, DqIssue, EmirRecord, FeedbackRecord, FeedbackType, Regime, Severity,
};

/// Check implementation.
pub struct TrInaccurateReported;

const CHECK_ID: &str = "EMIR.FBK.TR_INACCURATE_REPORTED";

impl FeedbackCheck for TrInaccurateReported {
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
        _prior: &[EmirRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        feedback
            .iter()
            .filter(|f| {
                matches!(f.regime, Regime::Emir) && f.feedback_type == FeedbackType::Inaccurate
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
                    regime: Regime::Emir,
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
    fn flags_inaccurate_feedback() {
        let feedback = vec![FeedbackRecord {
            regime: Regime::Emir,
            feedback_type: FeedbackType::Inaccurate,
            uti: Some("U1".into()),
            reported_field: Some("NotionalAmount".into()),
            reason_description: Some("Negative notional".into()),
            ..Default::default()
        }];
        let issues = TrInaccurateReported.run(&feedback, &[], &CheckContext::now_with_defaults());
        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("NotionalAmount"));
    }

    #[test]
    fn ignores_rejected_feedback() {
        let feedback = vec![FeedbackRecord {
            regime: Regime::Emir,
            feedback_type: FeedbackType::Rejected,
            uti: Some("U1".into()),
            ..Default::default()
        }];
        assert!(TrInaccurateReported
            .run(&feedback, &[], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
