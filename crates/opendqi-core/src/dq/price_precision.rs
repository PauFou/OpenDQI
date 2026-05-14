//! EMIR.VLD.PRICE_PRECISION — price must fit ESMA's `decimal:18.5`
//! precision.

use super::{Check, CheckContext};
use crate::dq::formats::within_decimal_bounds;
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct PricePrecision;

const CHECK_ID: &str = "EMIR.VLD.PRICE_PRECISION";

impl Check for PricePrecision {
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
                let p = r.price?;
                if within_decimal_bounds(&p, 18, 5) {
                    None
                } else {
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Emir,
                        severity: Severity::Warning,
                        dimension: DqDimension::Validity,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("price".into()),
                        value: Some(p.to_string()),
                        message: "Price exceeds ESMA decimal:18.5 precision.".into(),
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
    use rust_decimal::Decimal;
    use std::str::FromStr;
    #[test]
    fn flags_too_much_scale() {
        let r = EmirRecord {
            price: Some(Decimal::from_str("1.123456").unwrap()),
            ..Default::default()
        };
        assert_eq!(
            PricePrecision
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_normal() {
        let r = EmirRecord {
            price: Some(Decimal::from_str("100.25").unwrap()),
            ..Default::default()
        };
        assert!(PricePrecision
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
