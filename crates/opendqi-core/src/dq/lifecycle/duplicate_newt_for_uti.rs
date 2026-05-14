//! EMIR.LFC.DUPLICATE_NEWT_FOR_UTI — a NEWT on a UTI that already has
//! a NEWT in the history store is a lifecycle duplicate.

use crate::dq::lifecycle::modi_without_newt::index_priors_with_action;
use crate::dq::{CheckContext, LifecycleCheck};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct DuplicateNewtForUti;

const CHECK_ID: &str = "EMIR.LFC.DUPLICATE_NEWT_FOR_UTI";

impl LifecycleCheck for DuplicateNewtForUti {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Uniqueness
    }
    fn severity(&self) -> Severity {
        Severity::Critical
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
                .map(|a| !a.eq_ignore_ascii_case("NEWT"))
                .unwrap_or(true)
            {
                continue;
            }
            let uti = match r.uti.as_deref() {
                Some(u) if !u.trim().is_empty() => u.trim(),
                _ => continue,
            };
            if let Some(prevs) = prior_newt.get(uti) {
                out.push(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Emir,
                    severity: Severity::Critical,
                    dimension: DqDimension::Uniqueness,
                    record_id: r.record_id.clone(),
                    uti: Some(uti.to_owned()),
                    field: Some("action_type".into()),
                    value: Some("NEWT".into()),
                    message: format!(
                        "NEWT for UTI {uti} but the history store already records {n} prior NEWT(s) for this UTI.",
                        n = prevs.len()
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
    fn flags_duplicate_newt() {
        let current = vec![EmirRecord {
            uti: Some("U1".into()),
            action_type: Some("NEWT".into()),
            ..Default::default()
        }];
        let prior = vec![EmirRecord {
            uti: Some("U1".into()),
            action_type: Some("NEWT".into()),
            ..Default::default()
        }];
        let issues = DuplicateNewtForUti.run(&current, &prior, &CheckContext::now_with_defaults());
        assert_eq!(issues.len(), 1);
    }
    #[test]
    fn first_newt_accepted() {
        let current = vec![EmirRecord {
            uti: Some("U1".into()),
            action_type: Some("NEWT".into()),
            ..Default::default()
        }];
        let issues = DuplicateNewtForUti.run(&current, &[], &CheckContext::now_with_defaults());
        assert!(issues.is_empty());
    }
}
