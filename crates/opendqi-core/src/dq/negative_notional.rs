//! EMIR.ACC.NEGATIVE_NOTIONAL — a notional amount must be ≥ 0.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};
use rust_decimal::Decimal;

/// Check implementation.
pub struct NegativeNotional;

const CHECK_ID: &str = "EMIR.ACC.NEGATIVE_NOTIONAL";

impl Check for NegativeNotional {
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
                r.notional_amount
                    .map(|n| n < Decimal::ZERO)
                    .unwrap_or(false)
            })
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Emir,
                severity: Severity::High,
                dimension: DqDimension::Accuracy,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("notional_amount".into()),
                value: r.notional_amount.map(|d| d.to_string()),
                message: "Notional amount is negative.".into(),
                source_file: r.source_file.clone(),
                evidence: Vec::new(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_negative_notional() {
        let records = vec![
            EmirRecord {
                notional_amount: Some(Decimal::from(100)),
                ..Default::default()
            },
            EmirRecord {
                notional_amount: Some(Decimal::from(-1000)),
                ..Default::default()
            },
            EmirRecord {
                notional_amount: None,
                ..Default::default()
            },
        ];
        let ctx = CheckContext::now_with_defaults();
        let issues = NegativeNotional.run(&records, &ctx);
        assert_eq!(issues.len(), 1);
    }
}
