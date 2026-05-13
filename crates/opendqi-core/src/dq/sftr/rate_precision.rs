//! SFTR.VLD.RATE_PRECISION — rate fields (rebate rate, lending fee)
//! must fit `decimal:11.10` precision. Emits one issue per violating
//! field.

use super::SftrCheck;
use crate::dq::formats::within_decimal_bounds;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};

/// Check implementation.
pub struct SftrRatePrecision;

const CHECK_ID: &str = "SFTR.VLD.RATE_PRECISION";

impl SftrCheck for SftrRatePrecision {
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
        let mut out = Vec::new();
        for r in records {
            for (field, value) in [
                ("rebate_rate", r.rebate_rate),
                ("lending_fee", r.lending_fee),
            ] {
                if let Some(v) = value {
                    if !within_decimal_bounds(&v, 11, 10) {
                        out.push(DqIssue {
                            check_id: CHECK_ID.into(),
                            regime: Regime::Sftr,
                            severity: Severity::Warning,
                            dimension: DqDimension::Validity,
                            record_id: r.record_id.clone(),
                            uti: r.uti.clone(),
                            field: Some(field.into()),
                            value: Some(v.to_string()),
                            message: format!("{field} exceeds ESMA decimal:11.10 precision."),
                            source_file: r.source_file.clone(),
                        });
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use std::str::FromStr;
    #[test]
    fn flags_rebate_too_much_scale() {
        let r = SftrRecord {
            rebate_rate: Some(Decimal::from_str("0.12345678901").unwrap()),
            ..Default::default()
        };
        assert_eq!(
            SftrRatePrecision
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_normal_rates() {
        let r = SftrRecord {
            rebate_rate: Some(Decimal::from_str("0.0125").unwrap()),
            lending_fee: Some(Decimal::from_str("0.005").unwrap()),
            ..Default::default()
        };
        assert!(SftrRatePrecision
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
