//! SFTR.VLD.LOAN_PRECISION — loan value must fit ESMA's `decimal:18.5`
//! precision.

use super::SftrCheck;
use crate::dq::formats::within_decimal_bounds;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};

/// Check implementation.
pub struct SftrLoanPrecision;

const CHECK_ID: &str = "SFTR.VLD.LOAN_PRECISION";

impl SftrCheck for SftrLoanPrecision {
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
                let v = r.loan_value?;
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
                        field: Some("loan_value".into()),
                        value: Some(v.to_string()),
                        message: "Loan value exceeds ESMA decimal:18.5 precision.".into(),
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
    fn flags_too_many_int_digits() {
        let r = SftrRecord {
            loan_value: Some(Decimal::from_str("1234567890123456789").unwrap()),
            ..Default::default()
        };
        assert_eq!(
            SftrLoanPrecision
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_normal() {
        let r = SftrRecord {
            loan_value: Some(Decimal::from(1000000)),
            ..Default::default()
        };
        assert!(SftrLoanPrecision
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
