//! SFTR.VLD.COLLATERAL_PRECISION — collateral value must fit ESMA's
//! `decimal:18.5` precision.

use super::SftrCheck;
use crate::dq::formats::within_decimal_bounds;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};

/// Check implementation.
pub struct SftrCollateralPrecision;

const CHECK_ID: &str = "SFTR.VLD.COLLATERAL_PRECISION";

impl SftrCheck for SftrCollateralPrecision {
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
                let v = r.collateral_value?;
                if within_decimal_bounds(&v, 18, 5) {
                    None
                } else {
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Sftr,
                        severity: Severity::Warning,
                        dimension: DqDimension::Validity,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("collateral_value".into()),
                        value: Some(v.to_string()),
                        message: "Collateral value exceeds ESMA decimal:18.5 precision.".into(),
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
    use rust_decimal::Decimal;
    use std::str::FromStr;
    #[test]
    fn flags_too_much_scale() {
        let r = SftrRecord {
            collateral_value: Some(Decimal::from_str("1.234567").unwrap()),
            ..Default::default()
        };
        assert_eq!(
            SftrCollateralPrecision
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_normal() {
        let r = SftrRecord {
            collateral_value: Some(Decimal::from_str("1100.50").unwrap()),
            ..Default::default()
        };
        assert!(SftrCollateralPrecision
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
