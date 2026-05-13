//! EMIR.VLD.VALUATION_PRECISION_BY_CURRENCY — valuation amount must
//! respect the currency's natural decimal precision.

use super::{Check, CheckContext};
use crate::dq::formats::currency_max_scale;
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct ValuationPrecisionByCurrency;

const CHECK_ID: &str = "EMIR.VLD.VALUATION_PRECISION_BY_CURRENCY";

impl Check for ValuationPrecisionByCurrency {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Validity
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter_map(|r| {
                let amt = r.valuation_amount?;
                let ccy = r.valuation_currency.as_deref()?;
                let max_scale = currency_max_scale(ccy)?;
                if amt.scale() <= max_scale {
                    return None;
                }
                Some(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Emir,
                    severity: Severity::Warning,
                    dimension: DqDimension::Validity,
                    record_id: r.record_id.clone(),
                    uti: r.uti.clone(),
                    field: Some("valuation_amount".into()),
                    value: Some(amt.to_string()),
                    message: format!(
                        "Valuation amount {amt} has {actual} fractional digit(s) but currency {ccy} allows at most {max_scale}.",
                        actual = amt.scale(),
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
    use rust_decimal::Decimal;
    use std::str::FromStr;
    #[test]
    fn flags_jpy_with_decimals() {
        let r = EmirRecord {
            valuation_amount: Some(Decimal::from_str("1000.50").unwrap()),
            valuation_currency: Some("JPY".into()),
            ..Default::default()
        };
        assert_eq!(
            ValuationPrecisionByCurrency
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn accepts_eur_two_decimals() {
        let r = EmirRecord {
            valuation_amount: Some(Decimal::from_str("1000.50").unwrap()),
            valuation_currency: Some("EUR".into()),
            ..Default::default()
        };
        assert!(ValuationPrecisionByCurrency
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
