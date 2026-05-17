//! EMIR.WRN.CTRPTY_MISSING_VALUATION_HIGH — per-counterparty share of
//! outstanding derivatives with no valuation reported exceeds the
//! configured threshold (the `Wrnngs` breakdown of auth.106).

use super::{flag_rate_high_cp, thresholds, WarningsCounterpartyCheck};
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Severity, WarningsCounterpartyRecord};

/// Check implementation.
pub struct EmirWrnCtrptyMissingValuationHigh;

const CHECK_ID: &str = "EMIR.WRN.CTRPTY_MISSING_VALUATION_HIGH";

impl WarningsCounterpartyCheck for EmirWrnCtrptyMissingValuationHigh {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[WarningsCounterpartyRecord], ctx: &CheckContext) -> Vec<DqIssue> {
        flag_rate_high_cp(
            CHECK_ID,
            Severity::High,
            DqDimension::Completeness,
            "missing_valuation_rate",
            "Missing-valuation",
            records,
            |r| r.missing_valuation_rate,
            thresholds(ctx).missing_valuation_rate_max,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    #[test]
    fn flags_only_above_threshold_and_names_lei() {
        let recs = vec![
            WarningsCounterpartyRecord {
                counterparty_lei: Some("LEI-LOW".into()),
                missing_valuation_rate: Some(Decimal::from_str("0.02").unwrap()),
                ..Default::default()
            },
            WarningsCounterpartyRecord {
                counterparty_lei: Some("LEI-HIGH".into()),
                missing_valuation_rate: Some(Decimal::from_str("0.40").unwrap()),
                ..Default::default()
            },
            WarningsCounterpartyRecord {
                missing_valuation_rate: None,
                ..Default::default()
            },
        ];
        let ctx = CheckContext::now_with_defaults();
        let issues = EmirWrnCtrptyMissingValuationHigh.run(&recs, &ctx);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].check_id, CHECK_ID);
        assert!(issues[0].message.contains("LEI-HIGH"));
    }
}
