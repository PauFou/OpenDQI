//! EMIR.VLD.LEI_FORMAT_CCP — Central Counterparty LEI shape (ISO 17442).

use super::{Check, CheckContext};
use crate::dq::formats::is_valid_lei;
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct LeiFormatCcp;

const CHECK_ID: &str = "EMIR.VLD.LEI_FORMAT_CCP";

impl Check for LeiFormatCcp {
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
                let lei = r.clearing_ccp_lei.as_deref()?.trim();
                if lei.is_empty() || is_valid_lei(lei) {
                    None
                } else {
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Emir,
                        severity: Severity::High,
                        dimension: DqDimension::Validity,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("clearing_ccp_lei".into()),
                        value: Some(lei.to_owned()),
                        message: format!("CCP LEI '{lei}' is not a valid ISO 17442 identifier."),
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
    fn flags_bad_ccp_lei() {
        let r = EmirRecord {
            clearing_ccp_lei: Some("BAD".into()),
            ..Default::default()
        };
        assert_eq!(
            LeiFormatCcp
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn accepts_good_lei() {
        let r = EmirRecord {
            clearing_ccp_lei: Some("ABCDEFGHIJKLMNOPQR01".into()),
            ..Default::default()
        };
        assert!(LeiFormatCcp
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
