//! EMIR.VLD.NOTIONAL_PRECISION — notional amount must fit ESMA's
//! `decimal:18.5` precision.

use super::{Check, CheckContext};
use crate::dq::formats::within_decimal_bounds;
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct NotionalPrecision;

const CHECK_ID: &str = "EMIR.VLD.NOTIONAL_PRECISION";

impl Check for NotionalPrecision {
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
                let n = r.notional_amount?;
                if within_decimal_bounds(&n, 18, 5) {
                    None
                } else {
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Emir,
                        severity: Severity::Warning,
                        dimension: DqDimension::Validity,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("notional_amount".into()),
                        value: Some(n.to_string()),
                        message: "Notional amount exceeds ESMA decimal:18.5 precision.".into(),
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
    fn flags_too_many_int_digits() {
        let r = EmirRecord {
            notional_amount: Some(Decimal::from_str("1234567890123456789").unwrap()),
            ..Default::default()
        };
        assert_eq!(
            NotionalPrecision
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_normal_value() {
        let r = EmirRecord {
            notional_amount: Some(Decimal::from(1000000)),
            ..Default::default()
        };
        assert!(NotionalPrecision
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
