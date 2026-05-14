//! EMIR.LFC.ETRM_WITHOUT_NEWT — an early-termination action on a UTI
//! must be preceded by a NEWT for the same UTI in the history store.

use crate::dq::lifecycle::modi_without_newt::index_priors_with_action;
use crate::dq::{CheckContext, LifecycleCheck};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct EtrmWithoutNewt;

const CHECK_ID: &str = "EMIR.LFC.ETRM_WITHOUT_NEWT";

impl LifecycleCheck for EtrmWithoutNewt {
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
        current: &[EmirRecord],
        prior: &[EmirRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        let prior_newt = index_priors_with_action(prior, "NEWT");
        let mut out = Vec::new();
        for r in current {
            if r.action_type
                .as_deref()
                .map(|a| !a.eq_ignore_ascii_case("ETRM"))
                .unwrap_or(true)
            {
                continue;
            }
            let uti = match r.uti.as_deref() {
                Some(u) if !u.trim().is_empty() => u.trim(),
                _ => continue,
            };
            if !prior_newt.contains_key(uti) {
                out.push(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Emir,
                    severity: Severity::High,
                    dimension: DqDimension::Consistency,
                    record_id: r.record_id.clone(),
                    uti: Some(uti.to_owned()),
                    field: Some("action_type".into()),
                    value: Some("ETRM".into()),
                    message: format!(
                        "ETRM for UTI {uti} but no prior NEWT exists in the history store."
                    ),
                    source_file: r.source_file.clone(),
                    evidence: Vec::new(),
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
    fn flags_etrm_without_prior_newt() {
        let current = vec![EmirRecord {
            uti: Some("U1".into()),
            action_type: Some("ETRM".into()),
            ..Default::default()
        }];
        let issues = EtrmWithoutNewt.run(&current, &[], &CheckContext::now_with_defaults());
        assert_eq!(issues.len(), 1);
    }
    #[test]
    fn accepts_etrm_with_prior_newt() {
        let current = vec![EmirRecord {
            uti: Some("U1".into()),
            action_type: Some("ETRM".into()),
            ..Default::default()
        }];
        let prior = vec![EmirRecord {
            uti: Some("U1".into()),
            action_type: Some("NEWT".into()),
            ..Default::default()
        }];
        let issues = EtrmWithoutNewt.run(&current, &prior, &CheckContext::now_with_defaults());
        assert!(issues.is_empty());
    }
}
