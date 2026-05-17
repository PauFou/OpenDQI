//! SFTR.MCR.MISSING_UTI_ON_REQUEST — the auth.083 request omitted the
//! `UnqTradIdr`, so the firm cannot tie the request back to a specific
//! SFT in its books. Fires only when `uti` is `None`.

use super::{build_issue, counterparty_label, MissingCollateralCheck};
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, MissingCollateralRecord, Severity, SftrTrStateRecord};

/// Check implementation.
pub struct SftrMcrMissingUtiOnRequest;

const CHECK_ID: &str = "SFTR.MCR.MISSING_UTI_ON_REQUEST";

impl MissingCollateralCheck for SftrMcrMissingUtiOnRequest {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Validity
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(
        &self,
        records: &[MissingCollateralRecord],
        _tsr: Option<&[SftrTrStateRecord]>,
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| r.uti.is_none())
            .map(|r| {
                let mut issue = build_issue(CHECK_ID, Severity::High, DqDimension::Validity, r);
                issue.field = Some("UnqTradIdr".into());
                issue.message = format!(
                    "Missing-collateral request has no UTI ({}); cannot be matched to a booked SFT.",
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
    fn flags_only_records_without_uti() {
        let recs = vec![
            MissingCollateralRecord {
                uti: Some("UTI-1".into()),
                ..Default::default()
            },
            MissingCollateralRecord {
                uti: None,
                reporting_counterparty: Some("LEI-A".into()),
                ..Default::default()
            },
        ];
        let ctx = CheckContext::now_with_defaults();
        let issues = SftrMcrMissingUtiOnRequest.run(&recs, None, &ctx);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].check_id, CHECK_ID);
        assert_eq!(issues[0].field.as_deref(), Some("UnqTradIdr"));
    }
}
