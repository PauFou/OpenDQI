//! EMIR.VLD.MASTER_AGREEMENT_TYPE_ENUM — master agreement type must be
//! one of the ESMA-recognised codes (best-effort enumeration).

use super::{Check, CheckContext};
use crate::dq::formats::is_in;
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct MasterAgreementTypeEnum;

const CHECK_ID: &str = "EMIR.VLD.MASTER_AGREEMENT_TYPE_ENUM";
const ALLOWED: &[&str] = &[
    "ISDA", "FBF", "FMA", "EFET", "IETA", "IFEMA", "ICOM", "RPA", "EMA", "AFB", "EFMA", "GMRA",
    "GMSLA", "OTHR",
];

impl Check for MasterAgreementTypeEnum {
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
                let v = r.master_agreement_type.as_deref()?.trim();
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
                        field: Some("master_agreement_type".into()),
                        value: Some(v.to_owned()),
                        message: format!(
                            "Master agreement type '{v}' is not in the recognised enumeration."
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
    fn flags_unknown() {
        let r = EmirRecord {
            master_agreement_type: Some("XXX".into()),
            ..Default::default()
        };
        assert_eq!(
            MasterAgreementTypeEnum
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn accepts_isda() {
        let r = EmirRecord {
            master_agreement_type: Some("ISDA".into()),
            ..Default::default()
        };
        assert!(MasterAgreementTypeEnum
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
