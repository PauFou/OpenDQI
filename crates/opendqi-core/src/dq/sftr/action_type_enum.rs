//! SFTR.VLD.ACTION_TYPE_ENUM — action type must be one of the
//! standard SFTR action codes.

use super::SftrCheck;
use crate::dq::formats::is_in;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};

/// Check implementation.
pub struct SftrActionTypeEnum;

const CHECK_ID: &str = "SFTR.VLD.ACTION_TYPE_ENUM";
const ALLOWED: &[&str] = &[
    "NEWT", "MODI", "CORR", "ETRM", "VALU", "COLU", "MARU", "REUU", "POSC", "OTHR",
];

impl SftrCheck for SftrActionTypeEnum {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Validity
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[SftrRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter_map(|r| {
                let a = r.action_type.as_deref()?.trim();
                if a.is_empty() || is_in(a, ALLOWED) {
                    None
                } else {
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Sftr,
                        severity: Severity::High,
                        dimension: DqDimension::Validity,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("action_type".into()),
                        value: Some(a.to_owned()),
                        message: format!("Action type '{a}' is not a recognised SFTR code."),
                        source_file: r.source_file.clone(),
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
        let r = SftrRecord {
            action_type: Some("XXXX".into()),
            ..Default::default()
        };
        assert_eq!(
            SftrActionTypeEnum
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn accepts_modi() {
        let r = SftrRecord {
            action_type: Some("MODI".into()),
            ..Default::default()
        };
        assert!(SftrActionTypeEnum
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
