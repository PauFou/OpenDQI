//! EMIR.VLD.NATURE_ENUM — nature of the reporting counterparty must
//! be F (Financial), N (Non-Financial) or C (Central Counterparty).

use super::{Check, CheckContext};
use crate::dq::formats::is_in;
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct NatureEnum;

const CHECK_ID: &str = "EMIR.VLD.NATURE_ENUM";
const ALLOWED: &[&str] = &["F", "N", "C"];

impl Check for NatureEnum {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Validity
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter_map(|r| {
                let v = r.nature.as_deref()?.trim();
                if v.is_empty() || is_in(v, ALLOWED) {
                    None
                } else {
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Emir,
                        severity: Severity::Warning,
                        dimension: DqDimension::Validity,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("nature".into()),
                        value: Some(v.to_owned()),
                        message: format!("Nature '{v}' is not one of {{F, N, C}}."),
                        source_file: r.source_file.clone(),
                        evidence: Vec::new(),
                    })
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flags_unknown() {
        let r = EmirRecord {
            nature: Some("X".into()),
            ..Default::default()
        };
        assert_eq!(
            NatureEnum
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn accepts_known() {
        let r = EmirRecord {
            nature: Some("F".into()),
            ..Default::default()
        };
        assert!(NatureEnum
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
