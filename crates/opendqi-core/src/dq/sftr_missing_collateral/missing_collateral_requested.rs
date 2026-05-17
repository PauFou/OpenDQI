//! SFTR.MCR.MISSING_COLLATERAL_REQUESTED — the TR is requesting the
//! firm to provide the collateral missing for this SFT. One issue per
//! `TxId` in the auth.083 message; every record is actionable.

use super::{build_issue, counterparty_label, MissingCollateralCheck};
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, MissingCollateralRecord, Severity};

/// Check implementation.
pub struct SftrMcrMissingCollateralRequested;

const CHECK_ID: &str = "SFTR.MCR.MISSING_COLLATERAL_REQUESTED";

impl MissingCollateralCheck for SftrMcrMissingCollateralRequested {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[MissingCollateralRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .map(|r| {
                let mut issue = build_issue(CHECK_ID, Severity::High, DqDimension::Completeness, r);
                issue.message = format!(
                    "Trade Repository is requesting missing collateral for SFT {} ({}).",
                    r.uti.as_deref().unwrap_or("(no UTI)"),
                    counterparty_label(r),
                );
                issue
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_issue_per_record() {
        let recs = vec![
            MissingCollateralRecord {
                uti: Some("UTI-1".into()),
                ..Default::default()
            },
            MissingCollateralRecord {
                uti: None,
                ..Default::default()
            },
        ];
        let ctx = CheckContext::now_with_defaults();
        let issues = SftrMcrMissingCollateralRequested.run(&recs, &ctx);
        assert_eq!(issues.len(), 2);
        assert!(issues.iter().all(|i| i.check_id == CHECK_ID));
        assert_eq!(issues[0].uti.as_deref(), Some("UTI-1"));
    }
}
