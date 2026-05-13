//! EMIR.VLD.CLEARING_STATUS_ENUM — clearing status must be in the
//! allowed enumeration.

use super::{Check, CheckContext};
use crate::dq::formats::is_in;
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct ClearingStatusEnum;

const CHECK_ID: &str = "EMIR.VLD.CLEARING_STATUS_ENUM";
const ALLOWED: &[&str] = &["CLRD", "NCLR", "ICLR", "INCL"];

impl Check for ClearingStatusEnum {
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
                let v = r.clearing_status.as_deref()?.trim();
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
                        field: Some("clearing_status".into()),
                        value: Some(v.to_owned()),
                        message: format!(
                            "Clearing status '{v}' is not in the allowed set {{CLRD, NCLR, ICLR, INCL}}."
                        ),
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
    fn flags_unknown_status() {
        let r = EmirRecord {
            clearing_status: Some("UNKNOWN".into()),
            ..Default::default()
        };
        assert_eq!(
            ClearingStatusEnum
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn accepts_valid_status() {
        let r = EmirRecord {
            clearing_status: Some("CLRD".into()),
            ..Default::default()
        };
        assert!(ClearingStatusEnum
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
