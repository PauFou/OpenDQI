//! SFTR.TST.HAIRCUT_OUT_OF_RANGE_ON_OUTSTANDING — haircut < 0 or
//! > 1.0 on an outstanding SFT.

use rust_decimal::Decimal;

use super::{is_outstanding, SftrTrStateCheck};
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord, SftrTrStateRecord};

/// Check implementation.
pub struct SftrHaircutOutOfRangeOnTsr;

const CHECK_ID: &str = "SFTR.TST.HAIRCUT_OUT_OF_RANGE_ON_OUTSTANDING";

impl SftrTrStateCheck for SftrHaircutOutOfRangeOnTsr {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(
        &self,
        records: &[SftrTrStateRecord],
        _prior: &[SftrRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        let one = Decimal::ONE;
        records
            .iter()
            .filter_map(|r| {
                if !is_outstanding(r) {
                    return None;
                }
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
                        message: format!("Haircut {h} on outstanding SFT is outside [0, 1.0]."),
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
    fn flags_above_one() {
        let r = SftrTrStateRecord {
            uti: Some("U1".into()),
            status: Some("OUTSTANDING".into()),
            haircut: Some(Decimal::new(15, 1)),
            ..Default::default()
        };
        assert_eq!(
            SftrHaircutOutOfRangeOnTsr
                .run(&[r], &[], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn accepts_valid_haircut() {
        let r = SftrTrStateRecord {
            uti: Some("U1".into()),
            status: Some("OUTSTANDING".into()),
            haircut: Some(Decimal::new(5, 2)),
            ..Default::default()
        };
        assert!(SftrHaircutOutOfRangeOnTsr
            .run(&[r], &[], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
