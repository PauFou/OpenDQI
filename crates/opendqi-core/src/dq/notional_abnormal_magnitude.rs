//! EMIR.ACC.NOTIONAL_ABNORMAL_MAGNITUDE — a notional larger than
//! 10^15 (one quadrillion) is almost certainly a data-entry error.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};
use rust_decimal::Decimal;

/// Check implementation.
pub struct NotionalAbnormalMagnitude;

const CHECK_ID: &str = "EMIR.ACC.NOTIONAL_ABNORMAL_MAGNITUDE";

fn threshold() -> Decimal {
    Decimal::from(1_000_000_000_000_000i64)
}

impl Check for NotionalAbnormalMagnitude {
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
        let cap = threshold();
        records
            .iter()
            .filter_map(|r| {
                let n = r.notional_amount?;
                if n.abs() <= cap {
                    return None;
                }
                Some(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Emir,
                    severity: Severity::Warning,
                    dimension: DqDimension::Accuracy,
                    record_id: r.record_id.clone(),
                    uti: r.uti.clone(),
                    field: Some("notional_amount".into()),
                    value: Some(n.to_string()),
                    message: format!(
                        "Notional amount {n} exceeds the plausible cap of 10^15 — likely data-entry error."
                    ),
                    source_file: r.source_file.clone(),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    #[test]
    fn flags_above_cap() {
        let r = EmirRecord {
            notional_amount: Some(Decimal::from_str("9999999999999999").unwrap()),
            ..Default::default()
        };
        assert_eq!(
            NotionalAbnormalMagnitude
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_normal() {
        let r = EmirRecord {
            notional_amount: Some(Decimal::from(1_000_000)),
            ..Default::default()
        };
        assert!(NotionalAbnormalMagnitude
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
