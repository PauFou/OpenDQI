//! SFTR.CON.COLL_NEEDS_CURRENCY — a collateral value must carry its currency.

use super::SftrCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};

/// Check implementation.
pub struct SftrCollNeedsCurrency;

const CHECK_ID: &str = "SFTR.CON.COLL_NEEDS_CURRENCY";

impl SftrCheck for SftrCollNeedsCurrency {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[SftrRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| {
                r.collateral_value.is_some()
                    && r.collateral_currency
                        .as_deref()
                        .map(|s| s.trim().is_empty())
                        .unwrap_or(true)
            })
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Sftr,
                severity: Severity::High,
                dimension: DqDimension::Consistency,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("collateral_currency".into()),
                value: None,
                message: "Collateral value is reported but collateral currency is missing.".into(),
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
    fn flags_collateral_without_currency() {
        let r = SftrRecord {
            collateral_value: Some(Decimal::from(1100)),
            collateral_currency: None,
            ..Default::default()
        };
        assert_eq!(
            SftrCollNeedsCurrency
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_collateral_with_currency() {
        let r = SftrRecord {
            collateral_value: Some(Decimal::from(1100)),
            collateral_currency: Some("EUR".into()),
            ..Default::default()
        };
        assert!(SftrCollNeedsCurrency
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
