//! EMIR.FBK.TR_MISSING_DESPITE_SUBMISSION — the TR signals a UTI as
//! missing but the local history store records a prior NEWT for it.
//! Indicates either a TR ingestion failure or a stale feedback file.

use super::priors_with_newt;
use crate::dq::{CheckContext, FeedbackCheck};
use crate::model::{
    DqDimension, DqIssue, EmirRecord, FeedbackRecord, FeedbackType, Regime, Severity,
};

/// Check implementation.
pub struct TrMissingDespiteSubmission;

const CHECK_ID: &str = "EMIR.FBK.TR_MISSING_DESPITE_SUBMISSION";

impl FeedbackCheck for TrMissingDespiteSubmission {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::Critical
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
                if uti.is_empty() || !known.contains(uti) {
                    return None;
                }
                Some(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Emir,
                    severity: Severity::Critical,
                    dimension: DqDimension::Consistency,
                    record_id: f.record_id.clone(),
                    uti: Some(uti.to_owned()),
                    field: None,
                    value: None,
                    message: format!(
                        "TR reports UTI {uti} as missing, yet the local history store records a prior NEWT for it — TR ingestion failure or stale feedback."
                    ),
                    source_file: f.source_file.clone(),
                    evidence: Vec::new(),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_missing_with_prior_newt() {
        let feedback = vec![FeedbackRecord {
            regime: Regime::Emir,
            feedback_type: FeedbackType::Missing,
            uti: Some("U1".into()),
            ..Default::default()
        }];
        let prior = vec![EmirRecord {
            uti: Some("U1".into()),
            action_type: Some("NEWT".into()),
            ..Default::default()
        }];
        let issues =
            TrMissingDespiteSubmission.run(&feedback, &prior, &CheckContext::now_with_defaults());
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn ignores_missing_without_prior() {
        let feedback = vec![FeedbackRecord {
            regime: Regime::Emir,
            feedback_type: FeedbackType::Missing,
            uti: Some("U1".into()),
            ..Default::default()
        }];
        assert!(TrMissingDespiteSubmission
            .run(&feedback, &[], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
