//! EMIR.ACC.COMMODITY_REQUIRES_PRODUCT_ID — commodity derivatives
//! must report a product classification.

use super::{Check, CheckContext};
use crate::dq::formats::is_in;
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct CommodityRequiresProductId;

const CHECK_ID: &str = "EMIR.ACC.COMMODITY_REQUIRES_PRODUCT_ID";
const CO_CODES: &[&str] = &["CO", "COMM"];

impl Check for CommodityRequiresProductId {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| {
                r.asset_class
                    .as_deref()
                    .map(|s| is_in(s, CO_CODES))
                    .unwrap_or(false)
                    && r.product_id
                        .as_deref()
                        .map(str::trim)
                        .unwrap_or("")
                        .is_empty()
            })
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Emir,
                severity: Severity::Warning,
                dimension: DqDimension::Accuracy,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("product_id".into()),
                value: None,
                message: "Commodity derivative is missing a product identifier.".into(),
                source_file: r.source_file.clone(),
                evidence: Vec::new(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flags_commodity_without_product() {
        let r = EmirRecord {
            asset_class: Some("CO".into()),
            product_id: None,
            ..Default::default()
        };
        assert_eq!(
            CommodityRequiresProductId
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_commodity_with_product() {
        let r = EmirRecord {
            asset_class: Some("CO".into()),
            product_id: Some("XYZ".into()),
            ..Default::default()
        };
        assert!(CommodityRequiresProductId
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
