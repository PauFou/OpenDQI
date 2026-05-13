//! SFTR.ACC.NEGATIVE_LOAN — loan value must be non-negative.

use super::SftrCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};
use rust_decimal::Decimal;

/// Check implementation.
pub struct SftrNegativeLoan;

const CHECK_ID: &str = "SFTR.ACC.NEGATIVE_LOAN";

impl SftrCheck for SftrNegativeLoan {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[SftrRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| r.loan_value.map(|v| v < Decimal::ZERO).unwrap_or(false))
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Sftr,
                severity: Severity::High,
                dimension: DqDimension::Accuracy,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("loan_value".into()),
                value: r.loan_value.map(|d| d.to_string()),
                message: "Loan value is negative.".into(),
                source_file: r.source_file.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_negative_loan() {
        let records = vec![
            SftrRecord {
                loan_value: Some(Decimal::from(100)),
                ..Default::default()
            },
            SftrRecord {
                loan_value: Some(Decimal::from(-500)),
                ..Default::default()
            },
        ];
        let issues = SftrNegativeLoan.run(&records, &CheckContext::now_with_defaults());
        assert_eq!(issues.len(), 1);
    }
}
