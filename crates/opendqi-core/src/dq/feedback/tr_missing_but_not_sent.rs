//! EMIR.FBK.TR_MISSING_BUT_NOT_SENT — the TR signals a UTI as
//! missing and the local history store has no prior NEWT for it.
//! Confirms a genuine submission gap.

use super::priors_with_newt;
use crate::dq::{CheckContext, FeedbackCheck};
use crate::model::{
    DqDimension, DqIssue, EmirRecord, FeedbackRecord, FeedbackType, Regime, Severity,
};

/// Check implementation.
pub struct TrMissingButNotSent;

const CHECK_ID: &str = "EMIR.FBK.TR_MISSING_BUT_NOT_SENT";

impl FeedbackCheck for TrMissingButNotSent {
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
        prior: &[EmirRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        let known = priors_with_newt(prior);
        feedback
            .iter()
            .filter_map(|f| {
                if !matches!(f.regime, Regime::Emir) || f.feedback_type != FeedbackType::Missing {
                    return None;
                }
                let uti = f.uti.as_deref()?.trim();
                if uti.is_empty() || known.contains(uti) {
                    return None;
                }
                Some(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Emir,
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
    fn flags_missing_without_prior_newt() {
        let feedback = vec![FeedbackRecord {
            regime: Regime::Emir,
            feedback_type: FeedbackType::Missing,
            uti: Some("U-MISSING".into()),
            ..Default::default()
        }];
        let issues = TrMissingButNotSent.run(&feedback, &[], &CheckContext::now_with_defaults());
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn accepts_missing_when_prior_newt_exists() {
        let feedback = vec![FeedbackRecord {
            regime: Regime::Emir,
            feedback_type: FeedbackType::Missing,
            uti: Some("U-SENT".into()),
            ..Default::default()
        }];
        let prior = vec![EmirRecord {
            uti: Some("U-SENT".into()),
            action_type: Some("NEWT".into()),
            ..Default::default()
        }];
        assert!(TrMissingButNotSent
            .run(&feedback, &prior, &CheckContext::now_with_defaults())
            .is_empty());
    }
}
