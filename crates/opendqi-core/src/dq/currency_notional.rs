//! EMIR.VLD.CURRENCY_NOTIONAL — notional currency must be a
//! syntactically valid ISO 4217 code.

use super::{Check, CheckContext};
use crate::dq::formats::is_valid_currency_code;
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct CurrencyNotional;

const CHECK_ID: &str = "EMIR.VLD.CURRENCY_NOTIONAL";

impl Check for CurrencyNotional {
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
                let ccy = r.notional_currency.as_deref()?.trim();
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
                        field: Some("notional_currency".into()),
                        value: Some(ccy.to_owned()),
                        message: format!(
                            "Notional currency '{ccy}' is not a valid ISO 4217 code (3 uppercase letters)."
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
    fn flags_invalid_currency() {
        let records = vec![
            EmirRecord {
                notional_currency: Some("EUR".into()),
                ..Default::default()
            },
            EmirRecord {
                notional_currency: Some("eur".into()),
                ..Default::default()
            },
            EmirRecord {
                notional_currency: Some("EU".into()),
                ..Default::default()
            },
        ];
        let ctx = CheckContext::now_with_defaults();
        let issues = CurrencyNotional.run(&records, &ctx);
        assert_eq!(issues.len(), 2);
    }
}
