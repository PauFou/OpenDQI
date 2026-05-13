//! SFTR.REC.UNRECONCILED_TRADE — TR reports the SFT as UNRECONCILED.

use super::SftrReconciliationCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, ReconciliationRecord, Regime, Severity, SftrRecord};

/// Check implementation.
pub struct SftrUnreconciledTrade;

const CHECK_ID: &str = "SFTR.REC.UNRECONCILED_TRADE";

impl SftrReconciliationCheck for SftrUnreconciledTrade {
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
        records: &[ReconciliationRecord],
        _prior: &[SftrRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| {
                matches!(r.regime, Regime::Sftr)
                    && r.reconciliation_status
                        .as_deref()
                        .map(|s| s.eq_ignore_ascii_case("UNRECONCILED"))
                        .unwrap_or(false)
            })
            .map(|r| {
                let uti = r.uti.as_deref().unwrap_or("(unknown UTI)");
                let n = r.mismatched_fields.len();
                DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Sftr,
                    severity: Severity::High,
                    dimension: DqDimension::Consistency,
                    record_id: r.record_id.clone(),
                    uti: r.uti.clone(),
                    field: None,
                    value: r.reconciliation_status.clone(),
                    message: format!(
                        "TR reports UTI {uti} as UNRECONCILED — {n} field(s) disagree with the counterparty's submission."
                    ),
                    source_file: r.source_file.clone(),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_unreconciled() {
        let recs = vec![ReconciliationRecord {
            regime: Regime::Sftr,
            uti: Some("U1".into()),
            reconciliation_status: Some("UNRECONCILED".into()),
            ..Default::default()
        }];
        assert_eq!(
            SftrUnreconciledTrade
                .run(&recs, &[], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }

    #[test]
    fn ignores_reconciled() {
        let recs = vec![ReconciliationRecord {
            regime: Regime::Sftr,
            uti: Some("U1".into()),
            reconciliation_status: Some("RECONCILED".into()),
            ..Default::default()
        }];
        assert!(SftrUnreconciledTrade
            .run(&recs, &[], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
