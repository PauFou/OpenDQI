//! SFTR.VLD.HAIRCUT_PRECISION — haircut must fit `decimal:11.10`
//! (rate-style precision).

use super::SftrCheck;
use crate::dq::formats::within_decimal_bounds;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};

/// Check implementation.
pub struct SftrHaircutPrecision;

const CHECK_ID: &str = "SFTR.VLD.HAIRCUT_PRECISION";

impl SftrCheck for SftrHaircutPrecision {
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
                let v = r.haircut?;
                if within_decimal_bounds(&v, 11, 10) {
                    None
                } else {
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Sftr,
                        severity: Severity::Warning,
                        dimension: DqDimension::Validity,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("haircut".into()),
                        value: Some(v.to_string()),
                        message: "Haircut exceeds ESMA decimal:11.10 precision.".into(),
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
        let r = SftrRecord {
            haircut: Some(Decimal::from_str("0.12345678901").unwrap()),
            ..Default::default()
        };
        assert_eq!(
            SftrHaircutPrecision
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_normal() {
        let r = SftrRecord {
            haircut: Some(Decimal::from_str("0.05").unwrap()),
            ..Default::default()
        };
        assert!(SftrHaircutPrecision
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
