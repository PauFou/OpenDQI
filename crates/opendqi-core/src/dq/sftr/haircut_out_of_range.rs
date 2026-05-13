//! SFTR.ACC.HAIRCUT_OUT_OF_RANGE — haircut should fall within [0, 1.0].

use super::SftrCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};
use rust_decimal::Decimal;

/// Check implementation.
pub struct SftrHaircutOutOfRange;

const CHECK_ID: &str = "SFTR.ACC.HAIRCUT_OUT_OF_RANGE";

impl SftrCheck for SftrHaircutOutOfRange {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[SftrRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        let one = Decimal::ONE;
        records
            .iter()
            .filter_map(|r| {
                let h = r.haircut?;
                if h < Decimal::ZERO || h > one {
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Sftr,
                        severity: Severity::Warning,
                        dimension: DqDimension::Accuracy,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("haircut".into()),
                        value: Some(h.to_string()),
                        message: format!("Haircut {h} is outside the expected [0, 1.0] range."),
                        source_file: r.source_file.clone(),
                    })
                } else {
                    None
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_haircut_out_of_range() {
        let records = vec![
            SftrRecord {
                haircut: Some(Decimal::new(5, 2)), // 0.05
                ..Default::default()
            },
            SftrRecord {
                haircut: Some(Decimal::new(-1, 2)), // -0.01
                ..Default::default()
            },
            SftrRecord {
                haircut: Some(Decimal::new(15, 1)), // 1.5
                ..Default::default()
            },
        ];
        let issues = SftrHaircutOutOfRange.run(&records, &CheckContext::now_with_defaults());
        assert_eq!(issues.len(), 2);
    }
}
