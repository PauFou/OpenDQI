//! EMIR.LFC.MODI_WITHOUT_NEWT — a MODI / CORR action on a UTI must be
//! preceded by a NEWT for the same UTI in the history store.

use std::collections::HashMap;

use crate::dq::{CheckContext, LifecycleCheck};
use crate::model::{DqDimension, DqIssue, EmirRecord, EvidenceItem, Regime, Severity};

/// Check implementation.
pub struct ModiWithoutNewt;

const CHECK_ID: &str = "EMIR.LFC.MODI_WITHOUT_NEWT";

fn is_modification(a: &str) -> bool {
    a.eq_ignore_ascii_case("MODI") || a.eq_ignore_ascii_case("CORR")
}

impl LifecycleCheck for ModiWithoutNewt {
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
                    regime: Regime::Emir,
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
                    evidence: vec![EvidenceItem {
                        field: "action_type".into(),
                        before: None,
                        after: Some(action.to_owned()),
                        source_line: None,
                    }],
                });
            }
        }
        out
    }
}

pub(crate) fn index_priors_with_action<'a>(
    prior: &'a [EmirRecord],
    action: &str,
) -> HashMap<&'a str, Vec<&'a EmirRecord>> {
    let mut out: HashMap<&'a str, Vec<&'a EmirRecord>> = HashMap::new();
    for r in prior {
        if let (Some(uti), Some(act)) = (r.uti.as_deref(), r.action_type.as_deref()) {
            if act.eq_ignore_ascii_case(action) {
                out.entry(uti.trim()).or_default().push(r);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flags_modi_without_prior_newt() {
        let current = vec![EmirRecord {
            uti: Some("U1".into()),
            action_type: Some("MODI".into()),
            ..Default::default()
        }];
        let prior = vec![];
        let issues = ModiWithoutNewt.run(&current, &prior, &CheckContext::now_with_defaults());
        assert_eq!(issues.len(), 1);
        // Evidence pins down the action that violated the lifecycle.
        assert_eq!(issues[0].evidence.len(), 1);
        assert_eq!(issues[0].evidence[0].field, "action_type");
        assert_eq!(issues[0].evidence[0].after.as_deref(), Some("MODI"));
    }
    #[test]
    fn accepts_modi_when_prior_newt_exists() {
        let current = vec![EmirRecord {
            uti: Some("U1".into()),
            action_type: Some("MODI".into()),
            ..Default::default()
        }];
        let prior = vec![EmirRecord {
            uti: Some("U1".into()),
            action_type: Some("NEWT".into()),
            ..Default::default()
        }];
        let issues = ModiWithoutNewt.run(&current, &prior, &CheckContext::now_with_defaults());
        assert!(issues.is_empty());
    }
}
