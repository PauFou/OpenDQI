//! EMIR.VLD.COMMODITY_BASE_ENUM — for commodity-class trades, the
//! `product_id` should start with one of the ESMA commodity base
//! codes (AG agriculture, EN energy, FR freight, IN industrial,
//! OT other, EX exotic).

use super::{Check, CheckContext};
use crate::dq::formats::is_in;
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct CommodityBaseEnum;

const CHECK_ID: &str = "EMIR.VLD.COMMODITY_BASE_ENUM";
const COMMODITY_ASSET_CLASSES: &[&str] = &["CO", "COMM"];
const COMMODITY_BASE_CODES: &[&str] = &["AG", "EN", "FR", "IN", "OT", "EX"];

impl Check for CommodityBaseEnum {
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
                let asset_class = r.asset_class.as_deref()?.trim();
                if !is_in(asset_class, COMMODITY_ASSET_CLASSES) {
                    return None;
                }
                let product_id = r.product_id.as_deref()?.trim();
                if product_id.len() < 2 {
                    return None;
                }
                let prefix = &product_id[..2];
                if is_in(prefix, COMMODITY_BASE_CODES) {
                    return None;
                }
                Some(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Emir,
                    severity: Severity::Warning,
                    dimension: DqDimension::Validity,
                    record_id: r.record_id.clone(),
                    uti: r.uti.clone(),
                    field: Some("product_id".into()),
                    value: Some(product_id.to_owned()),
                    message: format!(
                        "Commodity product_id '{product_id}' does not start with a recognised base code (AG/EN/FR/IN/OT/EX)."
                    ),
                    source_file: r.source_file.clone(),
                    evidence: Vec::new(),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_unknown_prefix() {
        let r = EmirRecord {
            asset_class: Some("CO".into()),
            product_id: Some("ZZ123".into()),
            ..Default::default()
        };
        assert_eq!(
            CommodityBaseEnum
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }

    #[test]
    fn accepts_known_prefix() {
        let r = EmirRecord {
            asset_class: Some("CO".into()),
            product_id: Some("AGCORN".into()),
            ..Default::default()
        };
        assert!(CommodityBaseEnum
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }

    #[test]
    fn ignores_non_commodity() {
        let r = EmirRecord {
            asset_class: Some("IR".into()),
            product_id: Some("ZZ123".into()),
            ..Default::default()
        };
        assert!(CommodityBaseEnum
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
