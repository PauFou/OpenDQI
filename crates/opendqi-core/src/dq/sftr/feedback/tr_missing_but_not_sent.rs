//! SFTR.FBK.TR_MISSING_BUT_NOT_SENT — TR signals UTI as missing and
//! local history store has no prior NEWT for it.

use super::{priors_with_newt, SftrFeedbackCheck};
use crate::dq::CheckContext;
use crate::model::{
    DqDimension, DqIssue, FeedbackRecord, FeedbackType, Regime, Severity, SftrRecord,
};

/// Check implementation.
pub struct SftrTrMissingButNotSent;

const CHECK_ID: &str = "SFTR.FBK.TR_MISSING_BUT_NOT_SENT";

impl SftrFeedbackCheck for SftrTrMissingButNotSent {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(
        &self,
        feedback: &[FeedbackRecord],
        prior: &[SftrRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        let known = priors_with_newt(prior);
        feedback
            .iter()
            .filter_map(|f| {
                if !matches!(f.regime, Regime::Sftr) || f.feedback_type != FeedbackType::Missing {
                    return None;
                }
                let uti = f.uti.as_deref()?.trim();
                if uti.is_empty() || known.contains(uti) {
                    return None;
                }
                Some(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Sftr,
                    severity: Severity::High,
                    dimension: DqDimension::Completeness,
                    record_id: f.record_id.clone(),
                    uti: Some(uti.to_owned()),
                    field: None,
                    value: None,
                    message: format!(
                        "TR reports UTI {uti} as missing and no prior NEWT exists in the local history store — confirmed gap."
                    ),
                    source_file: f.source_file.clone(),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_when_no_prior() {
        let feedback = vec![FeedbackRecord {
            regime: Regime::Sftr,
            feedback_type: FeedbackType::Missing,
            uti: Some("U1".into()),
            ..Default::default()
        }];
        assert_eq!(
            SftrTrMissingButNotSent
                .run(&feedback, &[], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }

    #[test]
    fn ignores_when_prior_newt_exists() {
        let feedback = vec![FeedbackRecord {
            regime: Regime::Sftr,
            feedback_type: FeedbackType::Missing,
            uti: Some("U1".into()),
            ..Default::default()
        }];
        let prior = vec![SftrRecord {
            uti: Some("U1".into()),
            action_type: Some("NEWT".into()),
            ..Default::default()
        }];
        assert!(SftrTrMissingButNotSent
            .run(&feedback, &prior, &CheckContext::now_with_defaults())
            .is_empty());
    }
}
