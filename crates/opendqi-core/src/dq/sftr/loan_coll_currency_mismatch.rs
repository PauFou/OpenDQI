//! SFTR.CON.LOAN_COLL_CURRENCY_MISMATCH — loan and collateral
//! currencies differ. Legitimate for cross-currency SFTs but worth
//! surfacing.

use super::SftrCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};

/// Check implementation.
pub struct SftrLoanCollCurrencyMismatch;

const CHECK_ID: &str = "SFTR.CON.LOAN_COLL_CURRENCY_MISMATCH";

impl SftrCheck for SftrLoanCollCurrencyMismatch {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[SftrRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter_map(|r| {
                let l = r.loan_currency.as_deref()?.trim();
                let c = r.collateral_currency.as_deref()?.trim();
                if l.is_empty() || c.is_empty() || l.eq_ignore_ascii_case(c) {
                    None
                } else {
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Sftr,
                        severity: Severity::Warning,
                        dimension: DqDimension::Consistency,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("collateral_currency".into()),
                        value: Some(c.to_owned()),
                        message: format!(
                            "Collateral currency '{c}' differs from loan currency '{l}'."
                        ),
                        source_file: r.source_file.clone(),
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
        let r = SftrRecord {
            loan_currency: Some("EUR".into()),
            collateral_currency: Some("USD".into()),
            ..Default::default()
        };
        assert_eq!(
            SftrLoanCollCurrencyMismatch
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_match() {
        let r = SftrRecord {
            loan_currency: Some("EUR".into()),
            collateral_currency: Some("EUR".into()),
            ..Default::default()
        };
        assert!(SftrLoanCollCurrencyMismatch
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
