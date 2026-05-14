//! EMIR.VLD.ACTION_TYPE_ENUM — action type must be one of the
//! ESMA-defined codes.

use super::{Check, CheckContext};
use crate::dq::formats::is_in;
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct ActionTypeEnum;

const CHECK_ID: &str = "EMIR.VLD.ACTION_TYPE_ENUM";
const ALLOWED: &[&str] = &[
    "NEWT", "MODI", "CORR", "ETRM", "POSC", "VALU", "MARU", "OTHR", "EROR", "REVI",
];

impl Check for ActionTypeEnum {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Validity
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter_map(|r| {
                let v = r.action_type.as_deref()?.trim();
                if v.is_empty() || is_in(v, ALLOWED) {
                    None
                } else {
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Emir,
                        severity: Severity::High,
                        dimension: DqDimension::Validity,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("action_type".into()),
                        value: Some(v.to_owned()),
                        message: format!(
                            "Action type '{v}' is not in the ESMA-defined enumeration."
                        ),
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
    fn flags_unknown_action() {
        let r = EmirRecord {
            action_type: Some("FOOO".into()),
            ..Default::default()
        };
        assert_eq!(
            ActionTypeEnum
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn accepts_known_action() {
        let r = EmirRecord {
            action_type: Some("NEWT".into()),
            ..Default::default()
        };
        assert!(ActionTypeEnum
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
