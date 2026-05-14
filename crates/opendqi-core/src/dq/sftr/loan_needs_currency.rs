//! SFTR.CON.LOAN_NEEDS_CURRENCY — a loan value must carry its currency.

use super::SftrCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};

/// Check implementation.
pub struct SftrLoanNeedsCurrency;

const CHECK_ID: &str = "SFTR.CON.LOAN_NEEDS_CURRENCY";

impl SftrCheck for SftrLoanNeedsCurrency {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[SftrRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| {
                r.loan_value.is_some()
                    && r.loan_currency
                        .as_deref()
                        .map(|s| s.trim().is_empty())
                        .unwrap_or(true)
            })
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Sftr,
                severity: Severity::High,
                dimension: DqDimension::Consistency,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("loan_currency".into()),
                value: None,
                message: "Loan value is reported but loan currency is missing.".into(),
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
    fn flags_loan_without_currency() {
        let r = SftrRecord {
            loan_value: Some(Decimal::from(1000)),
            loan_currency: None,
            ..Default::default()
        };
        assert_eq!(
            SftrLoanNeedsCurrency
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_loan_with_currency() {
        let r = SftrRecord {
            loan_value: Some(Decimal::from(1000)),
            loan_currency: Some("EUR".into()),
            ..Default::default()
        };
        assert!(SftrLoanNeedsCurrency
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
