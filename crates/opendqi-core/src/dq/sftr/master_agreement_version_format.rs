//! SFTR.VLD.MASTER_AGREEMENT_VERSION_FORMAT — master agreement
//! version should be a 4-digit year.

use super::SftrCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};

/// Check implementation.
pub struct SftrMasterAgreementVersionFormat;

const CHECK_ID: &str = "SFTR.VLD.MASTER_AGREEMENT_VERSION_FORMAT";

fn is_four_digit_year(s: &str) -> bool {
    s.len() == 4 && s.bytes().all(|b| b.is_ascii_digit())
}

impl SftrCheck for SftrMasterAgreementVersionFormat {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Validity
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[SftrRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter_map(|r| {
                let v = r.master_agreement_version.as_deref()?.trim();
                if v.is_empty() || is_four_digit_year(v) {
                    None
                } else {
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Sftr,
                        severity: Severity::Warning,
                        dimension: DqDimension::Validity,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("master_agreement_version".into()),
                        value: Some(v.to_owned()),
                        message: format!("Master agreement version '{v}' is not a 4-digit year."),
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
    fn flags_non_year() {
        let r = SftrRecord {
            master_agreement_version: Some("v2.0".into()),
            ..Default::default()
        };
        assert_eq!(
            SftrMasterAgreementVersionFormat
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn accepts_year() {
        let r = SftrRecord {
            master_agreement_version: Some("2011".into()),
            ..Default::default()
        };
        assert!(SftrMasterAgreementVersionFormat
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
