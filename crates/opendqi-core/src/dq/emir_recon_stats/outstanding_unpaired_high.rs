//! EMIR.RST.OUTSTANDING_UNPAIRED_HIGH — flags counterparties whose
//! count of outstanding unpaired trades exceeds the configured ceiling
//! (default 1000).

use super::{build_issue, counterparty_label, reporting_date_label, thresholds, ReconStatsCheck};
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, ReconStatsRecord, Severity};

/// Check implementation.
pub struct EmirRstOutstandingUnpairedHigh;

const CHECK_ID: &str = "EMIR.RST.OUTSTANDING_UNPAIRED_HIGH";

impl ReconStatsCheck for EmirRstOutstandingUnpairedHigh {
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
        _prior: &[ReconStatsRecord],
        ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        let max = thresholds(ctx).outstanding_unpaired_max;
        records
            .iter()
            .filter_map(|r| {
                let n = r.outstanding_unpaired?;
                if n <= max {
                    return None;
                }
                let mut issue =
                    build_issue(CHECK_ID, Severity::Warning, DqDimension::Consistency, r);
                issue.field = Some("outstanding_unpaired".into());
                issue.value = Some(n.to_string());
                issue.message = format!(
                    "Outstanding unpaired count {n} > threshold {max} for counterparty {} on {}.",
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

    fn rec(cpty: &str, unpaired: i64) -> ReconStatsRecord {
        ReconStatsRecord {
            counterparty_lei: Some(cpty.into()),
            outstanding_unpaired: Some(unpaired),
            ..Default::default()
        }
    }

    #[test]
    fn flags_above_threshold_only() {
        let recs = vec![rec("A", 1500), rec("B", 200), rec("C", 1000)];
        let ctx = CheckContext::now_with_defaults();
        let issues = EmirRstOutstandingUnpairedHigh.run(&recs, &[], &ctx);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].check_id, "EMIR.RST.OUTSTANDING_UNPAIRED_HIGH");
    }
}
