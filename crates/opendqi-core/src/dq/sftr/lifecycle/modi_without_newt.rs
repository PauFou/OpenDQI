//! SFTR.LFC.MODI_WITHOUT_NEWT — a MODI / CORR action on a UTI must
//! be preceded by a NEWT for the same UTI in the history store.

use super::{index_priors_with_action, SftrLifecycleCheck};
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};

/// Check implementation.
pub struct SftrModiWithoutNewt;

const CHECK_ID: &str = "SFTR.LFC.MODI_WITHOUT_NEWT";

fn is_modification(a: &str) -> bool {
    a.eq_ignore_ascii_case("MODI") || a.eq_ignore_ascii_case("CORR")
}

impl SftrLifecycleCheck for SftrModiWithoutNewt {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(
        &self,
        current: &[SftrRecord],
        prior: &[SftrRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        let prior_newt = index_priors_with_action(prior, "NEWT");
        let mut out = Vec::new();
        for r in current {
            let action = match r.action_type.as_deref() {
                Some(a) => a,
                None => continue,
            };
            if !is_modification(action) {
                continue;
            }
            let uti = match r.uti.as_deref() {
                Some(u) if !u.trim().is_empty() => u.trim(),
                _ => continue,
            };
            if !prior_newt.contains_key(uti) {
                out.push(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Sftr,
                    severity: Severity::High,
                    dimension: DqDimension::Consistency,
                    record_id: r.record_id.clone(),
                    uti: Some(uti.to_owned()),
                    field: Some("action_type".into()),
                    value: Some(action.to_owned()),
                    message: format!(
                        "{action} for UTI {uti} but no prior NEWT exists in the history store."
                    ),
                    source_file: r.source_file.clone(),
                });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flags_modi_without_prior_newt() {
        let current = vec![SftrRecord {
            uti: Some("U1".into()),
            action_type: Some("MODI".into()),
            ..Default::default()
        }];
        let issues = SftrModiWithoutNewt.run(&current, &[], &CheckContext::now_with_defaults());
        assert_eq!(issues.len(), 1);
    }
    #[test]
    fn accepts_modi_with_prior_newt() {
        let current = vec![SftrRecord {
            uti: Some("U1".into()),
            action_type: Some("MODI".into()),
            ..Default::default()
        }];
        let prior = vec![SftrRecord {
            uti: Some("U1".into()),
            action_type: Some("NEWT".into()),
            ..Default::default()
        }];
        let issues = SftrModiWithoutNewt.run(&current, &prior, &CheckContext::now_with_defaults());
        assert!(issues.is_empty());
    }
}
