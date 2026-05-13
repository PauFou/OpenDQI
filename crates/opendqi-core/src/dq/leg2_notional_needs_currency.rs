//! EMIR.CON.LEG2_NOTIONAL_NEEDS_CURRENCY — a leg-2 notional amount must
//! carry its currency.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct Leg2NotionalNeedsCurrency;

const CHECK_ID: &str = "EMIR.CON.LEG2_NOTIONAL_NEEDS_CURRENCY";

impl Check for Leg2NotionalNeedsCurrency {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| {
                r.leg2_notional_amount.is_some()
                    && r.leg2_notional_currency
                        .as_deref()
                        .map(str::trim)
                        .unwrap_or("")
                        .is_empty()
            })
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Emir,
                severity: Severity::High,
                dimension: DqDimension::Consistency,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("leg2_notional_currency".into()),
                value: None,
                message: "Leg-2 notional amount is set but the currency is missing.".into(),
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
    fn flags_when_leg2_without_currency() {
        let r = EmirRecord {
            leg2_notional_amount: Some(Decimal::from(1)),
            leg2_notional_currency: None,
            ..Default::default()
        };
        assert_eq!(
            Leg2NotionalNeedsCurrency
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_with_currency() {
        let r = EmirRecord {
            leg2_notional_amount: Some(Decimal::from(1)),
            leg2_notional_currency: Some("USD".into()),
            ..Default::default()
        };
        assert!(Leg2NotionalNeedsCurrency
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
