//! EMIR.WRN.CTRPTY_ABNORMAL_VALUES_HIGH — per-counterparty share of
//! reported derivatives whose notional is an abnormal outlier exceeds
//! the configured threshold (the `Wrnngs` breakdown of auth.106).

use super::{flag_rate_high_cp, thresholds, WarningsCounterpartyCheck};
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Severity, WarningsCounterpartyRecord};

/// Check implementation.
pub struct EmirWrnCtrptyAbnormalValuesHigh;

const CHECK_ID: &str = "EMIR.WRN.CTRPTY_ABNORMAL_VALUES_HIGH";

impl WarningsCounterpartyCheck for EmirWrnCtrptyAbnormalValuesHigh {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[WarningsCounterpartyRecord], ctx: &CheckContext) -> Vec<DqIssue> {
        flag_rate_high_cp(
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
            WarningsCounterpartyRecord {
                counterparty_lei: Some("LEI-LOW".into()),
                abnormal_values_rate: Some(Decimal::from_str("0.005").unwrap()),
                ..Default::default()
            },
            WarningsCounterpartyRecord {
                counterparty_lei: Some("LEI-HIGH".into()),
                abnormal_values_rate: Some(Decimal::from_str("0.10").unwrap()),
                ..Default::default()
            },
        ];
        let ctx = CheckContext::now_with_defaults();
        let issues = EmirWrnCtrptyAbnormalValuesHigh.run(&recs, &ctx);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].check_id, CHECK_ID);
        assert!(issues[0].message.contains("LEI-HIGH"));
    }
}
