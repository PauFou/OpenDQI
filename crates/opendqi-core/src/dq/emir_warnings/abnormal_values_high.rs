//! EMIR.WRN.ABNORMAL_VALUES_HIGH — share of reported derivatives whose
//! notional is an abnormal outlier exceeds the configured threshold.

use super::{flag_rate_high, thresholds, WarningsCheck};
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Severity, TradeWarningsRecord};

/// Check implementation.
pub struct EmirWrnAbnormalValuesHigh;

const CHECK_ID: &str = "EMIR.WRN.ABNORMAL_VALUES_HIGH";

impl WarningsCheck for EmirWrnAbnormalValuesHigh {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[TradeWarningsRecord], ctx: &CheckContext) -> Vec<DqIssue> {
        flag_rate_high(
            CHECK_ID,
            Severity::High,
            DqDimension::Accuracy,
            "abnormal_values_rate",
            "Abnormal-values",
            records,
            |r| r.abnormal_values_rate,
            thresholds(ctx).abnormal_values_rate_max,
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
                abnormal_values_rate: Some(Decimal::from_str("0.005").unwrap()),
                ..Default::default()
            },
            TradeWarningsRecord {
                abnormal_values_rate: Some(Decimal::from_str("0.10").unwrap()),
                ..Default::default()
            },
        ];
        let ctx = CheckContext::now_with_defaults();
        let issues = EmirWrnAbnormalValuesHigh.run(&recs, &ctx);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].check_id, CHECK_ID);
    }
}
