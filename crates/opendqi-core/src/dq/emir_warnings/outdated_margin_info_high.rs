//! EMIR.WRN.OUTDATED_MARGIN_INFO_HIGH — share of outstanding
//! derivatives whose reported margin information is outdated
//! (>14 days) exceeds the configured threshold.

use super::{flag_rate_high, thresholds, WarningsCheck};
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Severity, TradeWarningsRecord};

/// Check implementation.
pub struct EmirWrnOutdatedMarginInfoHigh;

const CHECK_ID: &str = "EMIR.WRN.OUTDATED_MARGIN_INFO_HIGH";

impl WarningsCheck for EmirWrnOutdatedMarginInfoHigh {
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
            "outdated_margin_rate",
            "Outdated-margin-info",
            records,
            |r| r.outdated_margin_rate,
            thresholds(ctx).outdated_margin_rate_max,
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
                outdated_margin_rate: Some(Decimal::from_str("0.00").unwrap()),
                ..Default::default()
            },
            TradeWarningsRecord {
                outdated_margin_rate: Some(Decimal::from_str("0.20").unwrap()),
                ..Default::default()
            },
        ];
        let ctx = CheckContext::now_with_defaults();
        let issues = EmirWrnOutdatedMarginInfoHigh.run(&recs, &ctx);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].check_id, CHECK_ID);
    }
}
