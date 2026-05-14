//! EMIR.REC.FIELD_MISMATCH — emit one issue per field the TR flagged
//! as mismatched between the two counterparties' submissions.

use crate::dq::{CheckContext, ReconciliationCheck};
use crate::model::{
    DqDimension, DqIssue, EmirRecord, EvidenceItem, ReconciliationRecord, Regime, Severity,
};

/// Check implementation.
pub struct EmirFieldMismatch;

const CHECK_ID: &str = "EMIR.REC.FIELD_MISMATCH";

impl ReconciliationCheck for EmirFieldMismatch {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
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
        let mut out = Vec::new();
        for r in records {
            if !matches!(r.regime, Regime::Emir) {
                continue;
            }
            let uti = r.uti.as_deref();
            for field in &r.mismatched_fields {
                out.push(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Emir,
                    severity: Severity::High,
                    dimension: DqDimension::Accuracy,
                    record_id: r.record_id.clone(),
                    uti: r.uti.clone(),
                    field: Some(field.clone()),
                    value: None,
                    message: format!(
                        "TR reports field '{field}' on UTI {uti_disp} as mismatched with the counterparty.",
                        uti_disp = uti.unwrap_or("(unknown UTI)"),
                    ),
                    source_file: r.source_file.clone(),
                    evidence: vec![EvidenceItem {
                        field: field.clone(),
                        before: None,
                        after: None,
                        source_line: None,
                    }],
                });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_one_issue_per_field() {
        let recs = vec![ReconciliationRecord {
            regime: Regime::Emir,
            uti: Some("U1".into()),
            mismatched_fields: vec!["NotionalAmount".into(), "ValuationAmount".into()],
            ..Default::default()
        }];
        assert_eq!(
            EmirFieldMismatch
                .run(&recs, &[], &CheckContext::now_with_defaults())
                .len(),
            2
        );
    }

    #[test]
    fn empty_list_emits_nothing() {
        let recs = vec![ReconciliationRecord {
            regime: Regime::Emir,
            uti: Some("U1".into()),
            ..Default::default()
        }];
        assert!(EmirFieldMismatch
            .run(&recs, &[], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
