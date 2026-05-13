//! EMIR.VLD.MARGIN_PRECISION — every reported margin amount must fit
//! ESMA's `decimal:18.5` precision.

use super::{Check, CheckContext};
use crate::dq::formats::within_decimal_bounds;
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct MarginPrecision;

const CHECK_ID: &str = "EMIR.VLD.MARGIN_PRECISION";

impl Check for MarginPrecision {
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
                let candidates = [
                    ("initial_margin_posted", r.initial_margin_posted),
                    ("initial_margin_collected", r.initial_margin_collected),
                    ("variation_margin_posted", r.variation_margin_posted),
                    ("variation_margin_collected", r.variation_margin_collected),
                ];
                for (field, opt) in candidates {
                    if let Some(d) = opt {
                        if !within_decimal_bounds(&d, 18, 5) {
                            return Some(DqIssue {
                                check_id: CHECK_ID.into(),
                                regime: Regime::Emir,
                                severity: Severity::Warning,
                                dimension: DqDimension::Validity,
                                record_id: r.record_id.clone(),
                                uti: r.uti.clone(),
                                field: Some(field.into()),
                                value: Some(d.to_string()),
                                message: format!(
                                    "Margin field '{field}' exceeds ESMA decimal:18.5 precision."
                                ),
                                source_file: r.source_file.clone(),
                            });
                        }
                    }
                }
                None
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
    fn flags_when_im_posted_too_precise() {
        let r = EmirRecord {
            initial_margin_posted: Some(Decimal::from_str("1.123456").unwrap()),
            ..Default::default()
        };
        assert_eq!(
            MarginPrecision
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_normal_margins() {
        let r = EmirRecord {
            initial_margin_posted: Some(Decimal::from(100)),
            variation_margin_collected: Some(Decimal::from_str("0.05").unwrap()),
            ..Default::default()
        };
        assert!(MarginPrecision
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
