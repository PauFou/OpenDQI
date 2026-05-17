//! EMIR.WRN.CTRPTY_OUTDATED_MARGIN_INFO_HIGH — per-counterparty share
//! of outstanding derivatives whose margin information is outdated
//! exceeds the configured threshold (the `Wrnngs` breakdown of
//! auth.106).

use super::{flag_rate_high_cp, thresholds, WarningsCounterpartyCheck};
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Severity, WarningsCounterpartyRecord};

/// Check implementation.
pub struct EmirWrnCtrptyOutdatedMarginInfoHigh;

const CHECK_ID: &str = "EMIR.WRN.CTRPTY_OUTDATED_MARGIN_INFO_HIGH";

impl WarningsCounterpartyCheck for EmirWrnCtrptyOutdatedMarginInfoHigh {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Timeliness
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[WarningsCounterpartyRecord], ctx: &CheckContext) -> Vec<DqIssue> {
        flag_rate_high_cp(
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
            WarningsCounterpartyRecord {
                counterparty_lei: Some("LEI-LOW".into()),
                outdated_margin_rate: Some(Decimal::from_str("0.01").unwrap()),
                ..Default::default()
            },
            WarningsCounterpartyRecord {
                counterparty_lei: Some("LEI-HIGH".into()),
                outdated_margin_rate: Some(Decimal::from_str("0.25").unwrap()),
                ..Default::default()
            },
        ];
        let ctx = CheckContext::now_with_defaults();
        let issues = EmirWrnCtrptyOutdatedMarginInfoHigh.run(&recs, &ctx);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].check_id, CHECK_ID);
        assert!(issues[0].message.contains("LEI-HIGH"));
    }
}
