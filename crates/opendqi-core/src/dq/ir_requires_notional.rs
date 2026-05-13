//! EMIR.ACC.IR_REQUIRES_NOTIONAL — interest-rate trades must report
//! a notional amount.

use super::{Check, CheckContext};
use crate::dq::formats::is_in;
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct IrRequiresNotional;

const CHECK_ID: &str = "EMIR.ACC.IR_REQUIRES_NOTIONAL";
const IR_CODES: &[&str] = &["IR", "RATE"];

impl Check for IrRequiresNotional {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| {
                r.asset_class
                    .as_deref()
                    .map(|s| is_in(s, IR_CODES))
                    .unwrap_or(false)
                    && r.notional_amount.is_none()
            })
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Emir,
                severity: Severity::High,
                dimension: DqDimension::Accuracy,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("notional_amount".into()),
                value: None,
                message: "Interest-rate trade has no notional amount.".into(),
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
    fn flags_ir_without_notional() {
        let r = EmirRecord {
            asset_class: Some("IR".into()),
            notional_amount: None,
            ..Default::default()
        };
        assert_eq!(
            IrRequiresNotional
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_ir_with_notional() {
        let r = EmirRecord {
            asset_class: Some("IR".into()),
            notional_amount: Some(Decimal::from(1)),
            ..Default::default()
        };
        assert!(IrRequiresNotional
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
    #[test]
    fn ignores_non_ir() {
        let r = EmirRecord {
            asset_class: Some("FX".into()),
            notional_amount: None,
            ..Default::default()
        };
        assert!(IrRequiresNotional
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
