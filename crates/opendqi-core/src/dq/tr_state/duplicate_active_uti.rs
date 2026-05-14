//! EMIR.TST.DUPLICATE_ACTIVE_UTI — same UTI appears multiple times
//! among the outstanding rows of a TSR batch.

use std::collections::BTreeMap;

use super::is_outstanding;
use crate::dq::{CheckContext, TrStateCheck};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity, TrStateRecord};

/// Check implementation.
pub struct EmirDuplicateActiveUti;

const CHECK_ID: &str = "EMIR.TST.DUPLICATE_ACTIVE_UTI";

impl TrStateCheck for EmirDuplicateActiveUti {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Uniqueness
    }
    fn severity(&self) -> Severity {
        Severity::Critical
    }
    fn run(
        &self,
        records: &[TrStateRecord],
        _prior: &[EmirRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        let mut by_uti: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
        for (idx, r) in records.iter().enumerate() {
            if !is_outstanding(r) {
                continue;
            }
            if let Some(uti) = r.uti.as_deref() {
                let trimmed = uti.trim();
                if !trimmed.is_empty() {
                    by_uti.entry(trimmed).or_default().push(idx);
                }
            }
        }
        let mut out = Vec::new();
        for (uti, indices) in by_uti {
            if indices.len() <= 1 {
                continue;
            }
            for &idx in &indices {
                let r = &records[idx];
                out.push(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Emir,
                    severity: Severity::Critical,
                    dimension: DqDimension::Uniqueness,
                    record_id: r.record_id.clone(),
                    uti: Some(uti.to_owned()),
                    field: Some("uti".into()),
                    value: Some(uti.to_owned()),
                    message: format!(
                        "UTI {uti} appears {n} times as an outstanding trade in this TSR.",
                        n = indices.len()
                    ),
                    source_file: r.source_file.clone(),
                    evidence: Vec::new(),
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
    fn flags_duplicate_active() {
        let recs = vec![
            TrStateRecord {
                uti: Some("U1".into()),
                status: Some("OUTSTANDING".into()),
                ..Default::default()
            },
            TrStateRecord {
                uti: Some("U1".into()),
                status: Some("OUTSTANDING".into()),
                ..Default::default()
            },
            TrStateRecord {
                uti: Some("U2".into()),
                status: Some("OUTSTANDING".into()),
                ..Default::default()
            },
        ];
        let issues = EmirDuplicateActiveUti.run(&recs, &[], &CheckContext::now_with_defaults());
        assert_eq!(issues.len(), 2);
    }

    #[test]
    fn ignores_singletons() {
        let recs = vec![TrStateRecord {
            uti: Some("U1".into()),
            status: Some("OUTSTANDING".into()),
            ..Default::default()
        }];
        assert!(EmirDuplicateActiveUti
            .run(&recs, &[], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
