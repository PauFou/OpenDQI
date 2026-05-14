//! EMIR.RST.PAIRING_RATE_LOW — flags counterparties whose pairing
//! rate falls below the configured threshold (default 0.85).

use super::{
    build_issue, counterparty_label, decimal_to_f64, reporting_date_label, thresholds,
    ReconStatsCheck,
};
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, ReconStatsRecord, Severity};

/// Check implementation.
pub struct EmirRstPairingRateLow;

const CHECK_ID: &str = "EMIR.RST.PAIRING_RATE_LOW";

impl ReconStatsCheck for EmirRstPairingRateLow {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(
        &self,
        records: &[ReconStatsRecord],
        _prior: &[ReconStatsRecord],
        ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        let min = thresholds(ctx).pairing_rate_min;
        records
            .iter()
            .filter_map(|r| {
                let rate = r.pairing_rate.map(decimal_to_f64)?;
                if rate >= min {
                    return None;
                }
                let mut issue = build_issue(CHECK_ID, Severity::High, DqDimension::Consistency, r);
                issue.field = Some("pairing_rate".into());
                issue.value = Some(format!("{rate:.4}"));
                issue.message = format!(
                    "Pairing rate {:.2}% < threshold {:.2}% for counterparty {} on {}.",
                    rate * 100.0,
                    min * 100.0,
                    counterparty_label(r),
                    reporting_date_label(r),
                );
                Some(issue)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    fn rec(cpty: &str, rate: &str) -> ReconStatsRecord {
        ReconStatsRecord {
            counterparty_lei: Some(cpty.into()),
            pairing_rate: Some(Decimal::from_str(rate).unwrap()),
            ..Default::default()
        }
    }

    #[test]
    fn flags_below_threshold_only() {
        let recs = vec![rec("A", "0.95"), rec("B", "0.80"), rec("C", "0.85")];
        let ctx = CheckContext::now_with_defaults();
        let issues = EmirRstPairingRateLow.run(&recs, &[], &ctx);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].check_id, "EMIR.RST.PAIRING_RATE_LOW");
    }
}
