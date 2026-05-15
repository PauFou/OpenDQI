//! EMIR.FBK.TR_REJECTED_UTI — the Trade Repository rejected a
//! submission for this UTI at ingestion.

use crate::dq::{CheckContext, FeedbackCheck};
use crate::model::{
    DqDimension, DqIssue, EmirRecord, EvidenceItem, FeedbackRecord, FeedbackType, Regime, Severity,
};

/// Check implementation.
pub struct TrRejectedUti;

const CHECK_ID: &str = "EMIR.FBK.TR_REJECTED_UTI";

impl FeedbackCheck for TrRejectedUti {
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
        _prior: &[EmirRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        feedback
            .iter()
            .filter(|f| {
                matches!(f.regime, Regime::Emir) && f.feedback_type == FeedbackType::Rejected
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
                let mut message = format!("TR rejected report for UTI {uti}: {reason}{desc}");
                let mut evidence = Vec::new();
                if f.validation_rule_codes.len() > 1 {
                    let joined = f.validation_rule_codes.join(", ");
                    message.push_str(&format!(" (validation rules: {joined})"));
                    evidence.push(EvidenceItem {
                        field: "validation_rules".into(),
                        before: None,
                        after: Some(joined),
                        source_line: None,
                    });
                }
                DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Emir,
                    severity: Severity::Critical,
                    dimension: DqDimension::Validity,
                    record_id: f.record_id.clone(),
                    uti: f.uti.clone(),
                    field: None,
                    value: f.reason_code.clone(),
                    message,
                    source_file: f.source_file.clone(),
                    evidence,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_rejected_feedback() {
        let feedback = vec![FeedbackRecord {
            regime: Regime::Emir,
            feedback_type: FeedbackType::Rejected,
            uti: Some("U1".into()),
            reason_code: Some("VAL01".into()),
            reason_description: Some("Notional currency invalid".into()),
            ..Default::default()
        }];
        let issues = TrRejectedUti.run(&feedback, &[], &CheckContext::now_with_defaults());
        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("VAL01"));
    }

    #[test]
    fn ignores_other_feedback_types() {
        let feedback = vec![FeedbackRecord {
            regime: Regime::Emir,
            feedback_type: FeedbackType::Missing,
            uti: Some("U1".into()),
            ..Default::default()
        }];
        assert!(TrRejectedUti
            .run(&feedback, &[], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
