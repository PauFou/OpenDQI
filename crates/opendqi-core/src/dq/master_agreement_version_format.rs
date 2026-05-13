//! EMIR.VLD.MASTER_AGREEMENT_VERSION_FORMAT — master agreement
//! version should be a 4-digit year.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct MasterAgreementVersionFormat;

const CHECK_ID: &str = "EMIR.VLD.MASTER_AGREEMENT_VERSION_FORMAT";

fn is_four_digit_year(s: &str) -> bool {
    s.len() == 4 && s.bytes().all(|b| b.is_ascii_digit())
}

impl Check for MasterAgreementVersionFormat {
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
                let v = r.master_agreement_version.as_deref()?.trim();
                if v.is_empty() || is_four_digit_year(v) {
                    None
                } else {
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Emir,
                        severity: Severity::Warning,
                        dimension: DqDimension::Validity,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("master_agreement_version".into()),
                        value: Some(v.to_owned()),
                        message: format!("Master agreement version '{v}' is not a 4-digit year."),
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
    fn flags_non_year() {
        let r = EmirRecord {
            master_agreement_version: Some("v2".into()),
            ..Default::default()
        };
        assert_eq!(
            MasterAgreementVersionFormat
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn accepts_year() {
        let r = EmirRecord {
            master_agreement_version: Some("2002".into()),
            ..Default::default()
        };
        assert!(MasterAgreementVersionFormat
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
