//! EMIR.ACC.ZERO_NOTIONAL — a notional of zero is suspicious on
//! anything other than a position-component report.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};
use rust_decimal::Decimal;

/// Check implementation.
pub struct ZeroNotional;

const CHECK_ID: &str = "EMIR.ACC.ZERO_NOTIONAL";

impl Check for ZeroNotional {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| {
                // Position components legitimately carry zero.
                let action = r.action_type.as_deref().unwrap_or("");
                action != "POSC"
                    && r.notional_amount
                        .map(|n| n == Decimal::ZERO)
                        .unwrap_or(false)
            })
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Emir,
                severity: Severity::Warning,
                dimension: DqDimension::Accuracy,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("notional_amount".into()),
                value: Some("0".into()),
                message: "Notional amount is exactly zero on a non-position record.".into(),
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
    fn flags_zero_notional_on_new() {
        let records = vec![
            EmirRecord {
                action_type: Some("NEWT".into()),
                notional_amount: Some(Decimal::ZERO),
                ..Default::default()
            },
            EmirRecord {
                action_type: Some("POSC".into()),
                notional_amount: Some(Decimal::ZERO),
                ..Default::default()
            },
            EmirRecord {
                action_type: Some("NEWT".into()),
                notional_amount: Some(Decimal::from(100)),
                ..Default::default()
            },
        ];
        let ctx = CheckContext::now_with_defaults();
        let issues = ZeroNotional.run(&records, &ctx);
        assert_eq!(issues.len(), 1);
    }
}
