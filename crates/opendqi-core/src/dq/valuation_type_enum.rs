//! EMIR.VLD.VALUATION_TYPE_ENUM — valuation type must be MTMA or MTMO.

use super::{Check, CheckContext};
use crate::dq::formats::is_in;
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct ValuationTypeEnum;

const CHECK_ID: &str = "EMIR.VLD.VALUATION_TYPE_ENUM";
const ALLOWED: &[&str] = &["MTMA", "MTMO"];

impl Check for ValuationTypeEnum {
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
                let v = r.valuation_type.as_deref()?.trim();
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
                        field: Some("valuation_type".into()),
                        value: Some(v.to_owned()),
                        message: format!(
                            "Valuation type '{v}' is not in the allowed set {{MTMA, MTMO}}."
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
            valuation_type: Some("XX".into()),
            ..Default::default()
        };
        assert_eq!(
            ValuationTypeEnum
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn accepts_mtma() {
        let r = EmirRecord {
            valuation_type: Some("MTMA".into()),
            ..Default::default()
        };
        assert!(ValuationTypeEnum
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
