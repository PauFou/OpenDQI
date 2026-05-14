//! EMIR.COMP.VALUATION_CURRENCY_MISSING — a valuation amount must
//! carry a currency.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct ValuationCurrencyMissing;

const CHECK_ID: &str = "EMIR.COMP.VALUATION_CURRENCY_MISSING";

impl Check for ValuationCurrencyMissing {
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
                r.valuation_amount.is_some()
                    && r.valuation_currency
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
                field: Some("valuation_currency".into()),
                value: None,
                message: "Valuation amount is set but the currency is missing.".into(),
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
    fn flags_when_valuation_currency_absent() {
        let records = vec![
            EmirRecord {
                valuation_amount: Some(Decimal::from(100)),
                valuation_currency: Some("EUR".into()),
                ..Default::default()
            },
            EmirRecord {
                valuation_amount: Some(Decimal::from(200)),
                valuation_currency: None,
                ..Default::default()
            },
        ];
        let ctx = CheckContext::now_with_defaults();
        let issues = ValuationCurrencyMissing.run(&records, &ctx);
        assert_eq!(issues.len(), 1);
    }
}
