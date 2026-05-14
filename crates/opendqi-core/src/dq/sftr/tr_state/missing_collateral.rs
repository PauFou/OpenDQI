//! SFTR.TST.MISSING_COLLATERAL — outstanding SFT in the TSR with
//! no collateral value reported.

use super::{is_outstanding, SftrTrStateCheck};
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord, SftrTrStateRecord};

/// Check implementation.
pub struct SftrMissingCollateralOnTsr;

const CHECK_ID: &str = "SFTR.TST.MISSING_COLLATERAL";

impl SftrTrStateCheck for SftrMissingCollateralOnTsr {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(
        &self,
        records: &[SftrTrStateRecord],
        _prior: &[SftrRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| is_outstanding(r) && r.collateral_value.is_none())
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Sftr,
                severity: Severity::High,
                dimension: DqDimension::Completeness,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("collateral_value".into()),
                value: None,
                message: "TR shows the SFT as outstanding but no collateral value is reported."
                    .into(),
                source_file: r.source_file.clone(),
                evidence: Vec::new(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    #[test]
    fn flags_missing_collateral() {
        let r = SftrTrStateRecord {
            uti: Some("U1".into()),
            status: Some("OUTSTANDING".into()),
            collateral_value: None,
            ..Default::default()
        };
        assert_eq!(
            SftrMissingCollateralOnTsr
                .run(&[r], &[], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_when_collateral_present() {
        let r = SftrTrStateRecord {
            uti: Some("U1".into()),
            status: Some("OUTSTANDING".into()),
            collateral_value: Some(Decimal::from(1000)),
            ..Default::default()
        };
        assert!(SftrMissingCollateralOnTsr
            .run(&[r], &[], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
