//! EMIR.RST.PAIRING_RATE_TREND_DOWN — cross-batch: fires when the
//! pairing rate for a counterparty drops by more than 5 percentage
//! points compared with the latest prior batch loaded from the store.

use std::collections::HashMap;

use super::{
    build_issue, counterparty_label, decimal_to_f64, reporting_date_label, ReconStatsCheck,
};
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, ReconStatsRecord, Severity};

/// Check implementation.
pub struct EmirRstPairingRateTrendDown;

const CHECK_ID: &str = "EMIR.RST.PAIRING_RATE_TREND_DOWN";

/// Minimum drop (percentage points) that triggers an issue.
const DROP_PCT_THRESHOLD: f64 = 0.05;

impl ReconStatsCheck for EmirRstPairingRateTrendDown {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(
        &self,
        records: &[ReconStatsRecord],
        prior: &[ReconStatsRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        // Index prior batch by counterparty LEI (last value wins on dupes).
        let prior_by_cpty: HashMap<&str, f64> = prior
            .iter()
            .filter_map(|r| {
                let lei = r.counterparty_lei.as_deref()?;
                let rate = r.pairing_rate.map(decimal_to_f64)?;
                Some((lei, rate))
            })
            .collect();

        records
            .iter()
            .filter_map(|r| {
                let lei = r.counterparty_lei.as_deref()?;
                let current = r.pairing_rate.map(decimal_to_f64)?;
                let previous = prior_by_cpty.get(lei).copied()?;
                let drop = previous - current;
                if drop < DROP_PCT_THRESHOLD {
                    return None;
                }
                let mut issue =
                    build_issue(CHECK_ID, Severity::Warning, DqDimension::Consistency, r);
                issue.field = Some("pairing_rate".into());
                issue.value = Some(format!("{current:.4}"));
                issue.message = format!(
                    "Pairing rate dropped by {:.2}pp ({:.2}% → {:.2}%) for counterparty {} on {} vs previous batch.",
                    drop * 100.0,
                    previous * 100.0,
                    current * 100.0,
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
    fn flags_drop_exceeding_threshold() {
        let current = vec![rec("A", "0.80"), rec("B", "0.92")];
        let prior = vec![rec("A", "0.95"), rec("B", "0.93")];
        let ctx = CheckContext::now_with_defaults();
        let issues = EmirRstPairingRateTrendDown.run(&current, &prior, &ctx);
        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("dropped"));
    }

    #[test]
    fn empty_prior_emits_nothing() {
        let current = vec![rec("A", "0.80")];
        let ctx = CheckContext::now_with_defaults();
        let issues = EmirRstPairingRateTrendDown.run(&current, &[], &ctx);
        assert!(issues.is_empty());
    }
}
