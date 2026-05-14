//! EMIR.COMP.VALUATION_TIMESTAMP_MISSING — a valuation amount must
//! be accompanied by its valuation timestamp.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct ValuationTimestampMissing;

const CHECK_ID: &str = "EMIR.COMP.VALUATION_TIMESTAMP_MISSING";

impl Check for ValuationTimestampMissing {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| r.valuation_amount.is_some() && r.valuation_timestamp.is_none())
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Emir,
                severity: Severity::High,
                dimension: DqDimension::Completeness,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("valuation_timestamp".into()),
                value: None,
                message: "Valuation amount is set but the valuation timestamp is missing.".into(),
                source_file: r.source_file.clone(),
                evidence: Vec::new(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use rust_decimal::Decimal;

    #[test]
    fn flags_when_timestamp_absent() {
        let records = vec![
            EmirRecord {
                valuation_amount: Some(Decimal::from(1)),
                valuation_timestamp: Some(
                    chrono::Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap(),
                ),
                ..Default::default()
            },
            EmirRecord {
                valuation_amount: Some(Decimal::from(2)),
                valuation_timestamp: None,
                ..Default::default()
            },
        ];
        let ctx = CheckContext::now_with_defaults();
        let issues = ValuationTimestampMissing.run(&records, &ctx);
        assert_eq!(issues.len(), 1);
    }
}
