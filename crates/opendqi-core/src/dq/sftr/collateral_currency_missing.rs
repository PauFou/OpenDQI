//! SFTR.COMP.COLLATERAL_CURRENCY_MISSING — collateral value must carry a currency.

use super::SftrCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};

/// Check implementation.
pub struct SftrCollateralCurrencyMissing;

const CHECK_ID: &str = "SFTR.COMP.COLLATERAL_CURRENCY_MISSING";

impl SftrCheck for SftrCollateralCurrencyMissing {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[SftrRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| {
                r.collateral_value.is_some()
                    && r.collateral_currency
                        .as_deref()
                        .map(str::trim)
                        .unwrap_or("")
                        .is_empty()
            })
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Sftr,
                severity: Severity::High,
                dimension: DqDimension::Completeness,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("collateral_currency".into()),
                value: None,
                message: "Collateral value is set but the currency is missing.".into(),
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
    fn flags_collateral_without_currency() {
        let records = vec![
            SftrRecord {
                collateral_value: Some(Decimal::from(1000)),
                collateral_currency: Some("EUR".into()),
                ..Default::default()
            },
            SftrRecord {
                collateral_value: Some(Decimal::from(1500)),
                collateral_currency: None,
                ..Default::default()
            },
        ];
        let issues =
            SftrCollateralCurrencyMissing.run(&records, &CheckContext::now_with_defaults());
        assert_eq!(issues.len(), 1);
    }
}
