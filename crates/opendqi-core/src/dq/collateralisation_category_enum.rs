//! EMIR.VLD.COLLATERALISATION_CATEGORY_ENUM — collateralisation
//! category must be FLCL / OWCL / PRCL / UNCL.

use super::{Check, CheckContext};
use crate::dq::formats::is_in;
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct CollateralisationCategoryEnum;

const CHECK_ID: &str = "EMIR.VLD.COLLATERALISATION_CATEGORY_ENUM";
const ALLOWED: &[&str] = &["FLCL", "OWCL", "PRCL", "UNCL"];

impl Check for CollateralisationCategoryEnum {
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
                let v = r.collateralisation_category.as_deref()?.trim();
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
                        field: Some("collateralisation_category".into()),
                        value: Some(v.to_owned()),
                        message: format!(
                            "Collateralisation category '{v}' is not in the allowed set {{FLCL, OWCL, PRCL, UNCL}}."
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
    fn flags_unknown() {
        let r = EmirRecord {
            collateralisation_category: Some("XXX".into()),
            ..Default::default()
        };
        assert_eq!(
            CollateralisationCategoryEnum
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn accepts_flcl() {
        let r = EmirRecord {
            collateralisation_category: Some("FLCL".into()),
            ..Default::default()
        };
        assert!(CollateralisationCategoryEnum
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
