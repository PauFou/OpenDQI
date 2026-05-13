//! EMIR.REC.UNPAIRED_TRADE — TR reports the trade as UNPAIRED
//! (counterparty has not submitted a matching report).

use crate::dq::{CheckContext, ReconciliationCheck};
use crate::model::{DqDimension, DqIssue, EmirRecord, ReconciliationRecord, Regime, Severity};

/// Check implementation.
pub struct UnpairedTrade;

const CHECK_ID: &str = "EMIR.REC.UNPAIRED_TRADE";

impl ReconciliationCheck for UnpairedTrade {
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
                    && r.pairing_status
                        .as_deref()
                        .map(|s| s.eq_ignore_ascii_case("UNPAIRED"))
                        .unwrap_or(false)
            })
            .map(|r| {
                let uti = r.uti.as_deref().unwrap_or("(unknown UTI)");
                let other = r
                    .other_counterparty
                    .as_deref()
                    .unwrap_or("(no counterparty)");
                DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Emir,
                    severity: Severity::High,
                    dimension: DqDimension::Consistency,
                    record_id: r.record_id.clone(),
                    uti: r.uti.clone(),
                    field: None,
                    value: r.pairing_status.clone(),
                    message: format!(
                        "TR reports UTI {uti} as UNPAIRED — counterparty {other} has not submitted a matching report."
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
    fn flags_unpaired() {
        let recs = vec![ReconciliationRecord {
            regime: Regime::Emir,
            uti: Some("U1".into()),
            pairing_status: Some("UNPAIRED".into()),
            ..Default::default()
        }];
        assert_eq!(
            UnpairedTrade
                .run(&recs, &[], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }

    #[test]
    fn ignores_paired() {
        let recs = vec![ReconciliationRecord {
            regime: Regime::Emir,
            uti: Some("U1".into()),
            pairing_status: Some("PAIRED".into()),
            ..Default::default()
        }];
        assert!(UnpairedTrade
            .run(&recs, &[], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
