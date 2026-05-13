//! EMIR.COMP.NOTIONAL_CURRENCY_MISSING — a notional amount must
//! carry a currency.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct NotionalCurrencyMissing;

const CHECK_ID: &str = "EMIR.COMP.NOTIONAL_CURRENCY_MISSING";

impl Check for NotionalCurrencyMissing {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| {
                r.notional_amount.is_some()
                    && r.notional_currency
                        .as_deref()
                        .map(str::trim)
                        .unwrap_or("")
                        .is_empty()
            })
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Emir,
                severity: Severity::High,
                dimension: DqDimension::Completeness,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("notional_currency".into()),
                value: None,
                message: "Notional amount is set but the currency is missing.".into(),
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
    fn flags_when_currency_absent() {
        let records = vec![
            EmirRecord {
                notional_amount: Some(Decimal::from(1000)),
                notional_currency: Some("EUR".into()),
                ..Default::default()
            },
            EmirRecord {
                notional_amount: Some(Decimal::from(2000)),
                notional_currency: None,
                ..Default::default()
            },
            EmirRecord {
                // amount missing → not flagged
                notional_amount: None,
                notional_currency: None,
                ..Default::default()
            },
        ];
        let ctx = CheckContext::now_with_defaults();
        let issues = NotionalCurrencyMissing.run(&records, &ctx);
        assert_eq!(issues.len(), 1);
    }
}
