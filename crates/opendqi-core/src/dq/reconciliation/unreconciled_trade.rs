//! EMIR.REC.UNRECONCILED_TRADE — TR reports the trade as UNRECONCILED
//! (paired with the counterparty but fields disagree).

use crate::dq::{CheckContext, ReconciliationCheck};
use crate::model::{DqDimension, DqIssue, EmirRecord, ReconciliationRecord, Regime, Severity};

/// Check implementation.
pub struct UnreconciledTrade;

const CHECK_ID: &str = "EMIR.REC.UNRECONCILED_TRADE";

impl ReconciliationCheck for UnreconciledTrade {
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
        _prior: &[EmirRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| {
                matches!(r.regime, Regime::Emir)
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
                    regime: Regime::Emir,
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
    fn flags_unreconciled() {
        let recs = vec![ReconciliationRecord {
            regime: Regime::Emir,
            uti: Some("U1".into()),
            reconciliation_status: Some("UNRECONCILED".into()),
            mismatched_fields: vec!["NotionalAmount".into()],
            ..Default::default()
        }];
        assert_eq!(
            UnreconciledTrade
                .run(&recs, &[], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }

    #[test]
    fn ignores_reconciled() {
        let recs = vec![ReconciliationRecord {
            regime: Regime::Emir,
            uti: Some("U1".into()),
            reconciliation_status: Some("RECONCILED".into()),
            ..Default::default()
        }];
        assert!(UnreconciledTrade
            .run(&recs, &[], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
