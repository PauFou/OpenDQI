//! SFTR.TST.OUTSTANDING_SUMMARY — one Info issue per outstanding SFT.

use super::{is_outstanding, SftrTrStateCheck};
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord, SftrTrStateRecord};

/// Check implementation.
pub struct SftrOutstandingSummary;

const CHECK_ID: &str = "SFTR.TST.OUTSTANDING_SUMMARY";

impl SftrTrStateCheck for SftrOutstandingSummary {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::Info
    }
    fn run(
        &self,
        records: &[SftrTrStateRecord],
        _prior: &[SftrRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| is_outstanding(r))
            .map(|r| {
                let uti = r.uti.as_deref().unwrap_or("(no UTI)");
                let loan = r
                    .loan_value
                    .map(|n| format!("{} {}", n, r.loan_currency.as_deref().unwrap_or("?")))
                    .unwrap_or_else(|| "(no loan)".into());
                DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Sftr,
                    severity: Severity::Info,
                    dimension: DqDimension::Completeness,
                    record_id: r.record_id.clone(),
                    uti: r.uti.clone(),
                    field: None,
                    value: r.sft_type.clone(),
                    message: format!("Outstanding SFT UTI {uti}: loan={loan}."),
                    source_file: r.source_file.clone(),
                    evidence: Vec::new(),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn emits_one_per_outstanding() {
        let recs = vec![
            SftrTrStateRecord {
                uti: Some("U1".into()),
                status: Some("OUTSTANDING".into()),
                ..Default::default()
            },
            SftrTrStateRecord {
                uti: Some("U2".into()),
                status: Some("TERMINATED".into()),
                ..Default::default()
            },
        ];
        assert_eq!(
            SftrOutstandingSummary
                .run(&recs, &[], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
}
