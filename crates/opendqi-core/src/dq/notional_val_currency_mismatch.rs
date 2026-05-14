//! EMIR.CON.NOTIONAL_VAL_CURRENCY_MISMATCH — notional and valuation
//! should generally share a currency.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct NotionalValCurrencyMismatch;

const CHECK_ID: &str = "EMIR.CON.NOTIONAL_VAL_CURRENCY_MISMATCH";

impl Check for NotionalValCurrencyMismatch {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter_map(|r| {
                let n = r.notional_currency.as_deref()?.trim();
                let v = r.valuation_currency.as_deref()?.trim();
                if n.is_empty() || v.is_empty() || n.eq_ignore_ascii_case(v) {
                    None
                } else {
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Emir,
                        severity: Severity::Warning,
                        dimension: DqDimension::Consistency,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("valuation_currency".into()),
                        value: Some(v.to_owned()),
                        message: format!(
                            "Valuation currency '{v}' differs from notional currency '{n}'."
                        ),
                        source_file: r.source_file.clone(),
                        evidence: Vec::new(),
                    })
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flags_mismatch() {
        let r = EmirRecord {
            notional_currency: Some("EUR".into()),
            valuation_currency: Some("USD".into()),
            ..Default::default()
        };
        assert_eq!(
            NotionalValCurrencyMismatch
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_match() {
        let r = EmirRecord {
            notional_currency: Some("EUR".into()),
            valuation_currency: Some("EUR".into()),
            ..Default::default()
        };
        assert!(NotionalValCurrencyMismatch
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
