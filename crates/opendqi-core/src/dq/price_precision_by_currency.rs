//! EMIR.VLD.PRICE_PRECISION_BY_CURRENCY — price must respect the
//! currency's natural decimal precision.

use super::{Check, CheckContext};
use crate::dq::formats::currency_max_scale;
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct PricePrecisionByCurrency;

const CHECK_ID: &str = "EMIR.VLD.PRICE_PRECISION_BY_CURRENCY";

impl Check for PricePrecisionByCurrency {
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
                let amt = r.price?;
                let ccy = r.price_currency.as_deref()?;
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
                    field: Some("price".into()),
                    value: Some(amt.to_string()),
                    message: format!(
                        "Price {amt} has {actual} fractional digit(s) but currency {ccy} allows at most {max_scale}.",
                        actual = amt.scale(),
                    ),
                    source_file: r.source_file.clone(),
                    evidence: Vec::new(),
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
            price: Some(Decimal::from_str("100.25").unwrap()),
            price_currency: Some("JPY".into()),
            ..Default::default()
        };
        assert_eq!(
            PricePrecisionByCurrency
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn accepts_usd_two_decimals() {
        let r = EmirRecord {
            price: Some(Decimal::from_str("100.25").unwrap()),
            price_currency: Some("USD".into()),
            ..Default::default()
        };
        assert!(PricePrecisionByCurrency
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
