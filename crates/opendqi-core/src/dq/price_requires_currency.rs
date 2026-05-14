//! EMIR.CON.PRICE_REQUIRES_CURRENCY — a price must carry its currency.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct PriceRequiresCurrency;

const CHECK_ID: &str = "EMIR.CON.PRICE_REQUIRES_CURRENCY";

impl Check for PriceRequiresCurrency {
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
                r.price.is_some()
                    && r.price_currency
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
                field: Some("price_currency".into()),
                value: None,
                message: "Price is set but the price currency is missing.".into(),
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
    fn flags_missing_currency() {
        let r = EmirRecord {
            price: Some(Decimal::from(100)),
            price_currency: None,
            ..Default::default()
        };
        assert_eq!(
            PriceRequiresCurrency
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_with_currency() {
        let r = EmirRecord {
            price: Some(Decimal::from(100)),
            price_currency: Some("EUR".into()),
            ..Default::default()
        };
        assert!(PriceRequiresCurrency
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
