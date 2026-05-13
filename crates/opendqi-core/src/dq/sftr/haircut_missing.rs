//! SFTR.COMP.HAIRCUT_MISSING — once a collateral value is reported,
//! the corresponding haircut should be supplied too.

use super::SftrCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};

/// Check implementation.
pub struct SftrHaircutMissing;

const CHECK_ID: &str = "SFTR.COMP.HAIRCUT_MISSING";

impl SftrCheck for SftrHaircutMissing {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[SftrRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| r.collateral_value.is_some() && r.haircut.is_none())
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Sftr,
                severity: Severity::Warning,
                dimension: DqDimension::Completeness,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("haircut".into()),
                value: None,
                message: "Collateral value is set but the haircut is missing.".into(),
                source_file: r.source_file.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    #[test]
    fn flags_when_haircut_absent() {
        let records = vec![
            SftrRecord {
                uti: Some("A".into()),
                collateral_value: Some(Decimal::from(100)),
                haircut: Some(Decimal::new(5, 2)),
                ..Default::default()
            },
            SftrRecord {
                uti: Some("B".into()),
                collateral_value: Some(Decimal::from(100)),
                haircut: None,
                ..Default::default()
            },
        ];
        let ctx = CheckContext::now_with_defaults();
        let issues = SftrHaircutMissing.run(&records, &ctx);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].uti.as_deref(), Some("B"));
    }
}
