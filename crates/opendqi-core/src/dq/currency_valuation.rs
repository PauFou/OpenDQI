//! EMIR.VLD.CURRENCY_VALUATION — valuation currency must be a
//! syntactically valid ISO 4217 code.

use super::{Check, CheckContext};
use crate::dq::formats::is_valid_currency_code;
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct CurrencyValuation;

const CHECK_ID: &str = "EMIR.VLD.CURRENCY_VALUATION";

impl Check for CurrencyValuation {
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
                let ccy = r.valuation_currency.as_deref()?.trim();
                if ccy.is_empty() || is_valid_currency_code(ccy) {
                    None
                } else {
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Emir,
                        severity: Severity::Warning,
                        dimension: DqDimension::Validity,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("valuation_currency".into()),
                        value: Some(ccy.to_owned()),
                        message: format!(
                            "Valuation currency '{ccy}' is not a valid ISO 4217 code (3 uppercase letters)."
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
    fn flags_invalid_valuation_currency() {
        let records = vec![
            EmirRecord {
                valuation_currency: Some("EUR".into()),
                ..Default::default()
            },
            EmirRecord {
                valuation_currency: Some("EURO".into()),
                ..Default::default()
            },
        ];
        let ctx = CheckContext::now_with_defaults();
        let issues = CurrencyValuation.run(&records, &ctx);
        assert_eq!(issues.len(), 1);
    }
}
