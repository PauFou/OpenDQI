//! EMIR.COMP.VALUATION_MISSING — outstanding trades must carry a valuation.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct MissingValuation;

const CHECK_ID: &str = "EMIR.COMP.VALUATION_MISSING";

impl Check for MissingValuation {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[EmirRecord], ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| r.is_outstanding(ctx.today) && r.valuation_amount.is_none())
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Emir,
                severity: Severity::High,
                dimension: DqDimension::Completeness,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("valuation_amount".into()),
                value: None,
                message: "Outstanding trade has no valuation amount.".into(),
                source_file: r.source_file.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use rust_decimal::Decimal;

    fn ctx() -> CheckContext {
        let mut c = CheckContext::now_with_defaults();
        c.today = NaiveDate::from_ymd_opt(2026, 5, 13).unwrap();
        c
    }

    #[test]
    fn flags_outstanding_without_valuation() {
        let records = vec![
            EmirRecord {
                uti: Some("A".into()),
                valuation_amount: Some(Decimal::new(1, 0)),
                ..Default::default()
            },
            EmirRecord {
                uti: Some("B".into()),
                valuation_amount: None,
                ..Default::default()
            },
            // terminated yesterday → not outstanding → not flagged
            EmirRecord {
                uti: Some("C".into()),
                valuation_amount: None,
                termination_date: NaiveDate::from_ymd_opt(2026, 5, 12),
                ..Default::default()
            },
        ];
        let issues = MissingValuation.run(&records, &ctx());
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].uti.as_deref(), Some("B"));
    }
}
