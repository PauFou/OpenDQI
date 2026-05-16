//! EMIR.WRN.OUTDATED_VALUATION_HIGH — share of outstanding derivatives
//! whose reported valuation is outdated (>14 days) exceeds threshold.

use super::{flag_rate_high, thresholds, WarningsCheck};
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Severity, TradeWarningsRecord};

/// Check implementation.
pub struct EmirWrnOutdatedValuationHigh;

const CHECK_ID: &str = "EMIR.WRN.OUTDATED_VALUATION_HIGH";

impl WarningsCheck for EmirWrnOutdatedValuationHigh {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Timeliness
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[TradeWarningsRecord], ctx: &CheckContext) -> Vec<DqIssue> {
        flag_rate_high(
            CHECK_ID,
            Severity::High,
            DqDimension::Timeliness,
            "outdated_valuation_rate",
            "Outdated-valuation",
            records,
            |r| r.outdated_valuation_rate,
            thresholds(ctx).outdated_valuation_rate_max,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    #[test]
    fn flags_only_above_threshold() {
        let recs = vec![
            TradeWarningsRecord {
                outdated_valuation_rate: Some(Decimal::from_str("0.01").unwrap()),
                ..Default::default()
            },
            TradeWarningsRecord {
                outdated_valuation_rate: Some(Decimal::from_str("0.30").unwrap()),
                ..Default::default()
            },
        ];
        let ctx = CheckContext::now_with_defaults();
        let issues = EmirWrnOutdatedValuationHigh.run(&recs, &ctx);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].check_id, CHECK_ID);
    }
}
