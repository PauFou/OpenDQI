//! SFTR.COMP.COLLATERAL_VALUE_MISSING — outstanding SFTs must carry
//! a collateral value.

use super::SftrCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};

/// Check implementation.
pub struct SftrCollateralValueMissing;

const CHECK_ID: &str = "SFTR.COMP.COLLATERAL_VALUE_MISSING";

impl SftrCheck for SftrCollateralValueMissing {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[SftrRecord], ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| r.is_outstanding(ctx.today) && r.collateral_value.is_none())
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Sftr,
                severity: Severity::High,
                dimension: DqDimension::Completeness,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("collateral_value".into()),
                value: None,
                message: "Outstanding SFT has no collateral value.".into(),
                source_file: r.source_file.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use rust_decimal::Decimal;

    fn ctx() -> CheckContext {
        let mut c = CheckContext::now_with_defaults();
        c.today = NaiveDate::from_ymd_opt(2026, 5, 13).unwrap();
        c
    }

    #[test]
    fn flags_outstanding_without_collateral() {
        let records = vec![
            SftrRecord {
                uti: Some("A".into()),
                collateral_value: Some(Decimal::from(1000)),
                ..Default::default()
            },
            SftrRecord {
                uti: Some("B".into()),
                collateral_value: None,
                ..Default::default()
            },
            // terminated → not outstanding → not flagged
            SftrRecord {
                uti: Some("C".into()),
                collateral_value: None,
                termination_date: NaiveDate::from_ymd_opt(2026, 5, 1),
                ..Default::default()
            },
        ];
        let issues = SftrCollateralValueMissing.run(&records, &ctx());
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].uti.as_deref(), Some("B"));
    }
}
