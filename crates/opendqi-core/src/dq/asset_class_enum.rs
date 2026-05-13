//! EMIR.VLD.ASSET_CLASS_ENUM — asset class must be one of the EMIR
//! six top-level codes.

use super::{Check, CheckContext};
use crate::dq::formats::is_in;
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct AssetClassEnum;

const CHECK_ID: &str = "EMIR.VLD.ASSET_CLASS_ENUM";
const ALLOWED: &[&str] = &[
    "CR", "CU", "EQ", "FX", "IR", "CO", "RATE", "COMM", "EQTY", "CRDT",
];

impl Check for AssetClassEnum {
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
                let v = r.asset_class.as_deref()?.trim();
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
                        field: Some("asset_class".into()),
                        value: Some(v.to_owned()),
                        message: format!("Asset class '{v}' is not in the EMIR enumeration."),
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
            asset_class: Some("BOGUS".into()),
            ..Default::default()
        };
        assert_eq!(
            AssetClassEnum
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn accepts_known() {
        let r = EmirRecord {
            asset_class: Some("IR".into()),
            ..Default::default()
        };
        assert!(AssetClassEnum
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
