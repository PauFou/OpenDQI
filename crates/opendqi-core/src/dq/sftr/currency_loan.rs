//! SFTR.VLD.CURRENCY_LOAN — loan currency shape (ISO 4217).

use super::SftrCheck;
use crate::dq::formats::is_valid_currency_code;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};

/// Check implementation.
pub struct SftrCurrencyLoan;

const CHECK_ID: &str = "SFTR.VLD.CURRENCY_LOAN";

impl SftrCheck for SftrCurrencyLoan {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Validity
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[SftrRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter_map(|r| {
                let ccy = r.loan_currency.as_deref()?.trim();
                if ccy.is_empty() || is_valid_currency_code(ccy) {
                    None
                } else {
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Sftr,
                        severity: Severity::Warning,
                        dimension: DqDimension::Validity,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("loan_currency".into()),
                        value: Some(ccy.to_owned()),
                        message: format!(
                            "Loan currency '{ccy}' is not a valid ISO 4217 code (3 uppercase letters)."
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
    fn flags_invalid_loan_currency() {
        let records = vec![
            SftrRecord {
                loan_currency: Some("EUR".into()),
                ..Default::default()
            },
            SftrRecord {
                loan_currency: Some("eur".into()),
                ..Default::default()
            },
        ];
        let ctx = CheckContext::now_with_defaults();
        let issues = SftrCurrencyLoan.run(&records, &ctx);
        assert_eq!(issues.len(), 1);
    }
}
