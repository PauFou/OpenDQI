//! EMIR.WRN.MISSING_MARGIN_INFO_HIGH — share of outstanding
//! derivatives with no margin information reported exceeds threshold.

use super::{flag_rate_high, thresholds, WarningsCheck};
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Severity, TradeWarningsRecord};

/// Check implementation.
pub struct EmirWrnMissingMarginInfoHigh;

const CHECK_ID: &str = "EMIR.WRN.MISSING_MARGIN_INFO_HIGH";

impl WarningsCheck for EmirWrnMissingMarginInfoHigh {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[TradeWarningsRecord], ctx: &CheckContext) -> Vec<DqIssue> {
        flag_rate_high(
            CHECK_ID,
            Severity::High,
            DqDimension::Completeness,
            "missing_margin_rate",
            "Missing-margin-info",
            records,
            |r| r.missing_margin_rate,
            thresholds(ctx).missing_margin_rate_max,
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
                missing_margin_rate: Some(Decimal::from_str("0.03").unwrap()),
                ..Default::default()
            },
            TradeWarningsRecord {
                missing_margin_rate: Some(Decimal::from_str("0.50").unwrap()),
                ..Default::default()
            },
        ];
        let ctx = CheckContext::now_with_defaults();
        let issues = EmirWrnMissingMarginInfoHigh.run(&recs, &ctx);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].check_id, CHECK_ID);
    }
}
