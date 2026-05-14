//! EMIR.CON.MODI_PRESERVES_UTI — when a MODI/CORR carries a
//! `prior_uti`, it must differ from the current `uti`. A modification
//! that keeps the same identifier as its prior is an oxymoron — UTI
//! re-statements are NOVA / CORR actions, not pure MODI.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, EvidenceItem, Regime, Severity};

/// Check implementation.
pub struct ModiPreservesUti;

const CHECK_ID: &str = "EMIR.CON.MODI_PRESERVES_UTI";

fn is_modification(a: &str) -> bool {
    a.eq_ignore_ascii_case("MODI") || a.eq_ignore_ascii_case("CORR")
}

impl Check for ModiPreservesUti {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter_map(|r| {
                let action = r.action_type.as_deref()?.trim();
                if !is_modification(action) {
                    return None;
                }
                let uti = r.uti.as_deref()?.trim();
                let prior = r.prior_uti.as_deref()?.trim();
                if uti.is_empty() || prior.is_empty() || !uti.eq_ignore_ascii_case(prior) {
                    return None;
                }
                Some(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Emir,
                    severity: Severity::Warning,
                    dimension: DqDimension::Consistency,
                    record_id: r.record_id.clone(),
                    uti: Some(uti.to_owned()),
                    field: Some("prior_uti".into()),
                    value: Some(prior.to_owned()),
                    message: format!(
                        "{action} for UTI {uti} carries a prior_uti identical to its current UTI — no-op."
                    ),
                    source_file: r.source_file.clone(),
                    evidence: vec![EvidenceItem {
                        field: "uti".into(),
                        before: Some(prior.to_owned()),
                        after: Some(uti.to_owned()),
                        source_line: None,
                    }],
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flags_identical_uti_prior() {
        let r = EmirRecord {
            action_type: Some("MODI".into()),
            uti: Some("U1".into()),
            prior_uti: Some("U1".into()),
            ..Default::default()
        };
        let issues = ModiPreservesUti.run(&[r], &CheckContext::now_with_defaults());
        assert_eq!(issues.len(), 1);
        // Evidence captures the before/after UTI pair.
        assert_eq!(issues[0].evidence.len(), 1);
        assert_eq!(issues[0].evidence[0].field, "uti");
        assert_eq!(issues[0].evidence[0].before.as_deref(), Some("U1"));
        assert_eq!(issues[0].evidence[0].after.as_deref(), Some("U1"));
    }
    #[test]
    fn ignores_different_prior() {
        let r = EmirRecord {
            action_type: Some("MODI".into()),
            uti: Some("U1".into()),
            prior_uti: Some("OLD-U1".into()),
            ..Default::default()
        };
        assert!(ModiPreservesUti
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
    #[test]
    fn ignores_newt() {
        let r = EmirRecord {
            action_type: Some("NEWT".into()),
            uti: Some("U1".into()),
            prior_uti: Some("U1".into()),
            ..Default::default()
        };
        assert!(ModiPreservesUti
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
