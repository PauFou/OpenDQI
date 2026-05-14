//! SFTR.TST.DUPLICATE_ACTIVE_UTI — same UTI appears multiple times
//! among the outstanding rows of an SFTR TSR batch.

use std::collections::BTreeMap;

use super::{is_outstanding, SftrTrStateCheck};
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord, SftrTrStateRecord};

/// Check implementation.
pub struct SftrDuplicateActiveUti;

const CHECK_ID: &str = "SFTR.TST.DUPLICATE_ACTIVE_UTI";

impl SftrTrStateCheck for SftrDuplicateActiveUti {
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
        records: &[SftrTrStateRecord],
        _prior: &[SftrRecord],
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
                    regime: Regime::Sftr,
                    severity: Severity::Critical,
                    dimension: DqDimension::Uniqueness,
                    record_id: r.record_id.clone(),
                    uti: Some(uti.to_owned()),
                    field: Some("uti".into()),
                    value: Some(uti.to_owned()),
                    message: format!(
                        "UTI {uti} appears {n} times as outstanding in this SFTR TSR.",
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
    fn flags_duplicate() {
        let recs = vec![
            SftrTrStateRecord {
                uti: Some("U1".into()),
                status: Some("OUTSTANDING".into()),
                ..Default::default()
            },
            SftrTrStateRecord {
                uti: Some("U1".into()),
                status: Some("OUTSTANDING".into()),
                ..Default::default()
            },
        ];
        assert_eq!(
            SftrDuplicateActiveUti
                .run(&recs, &[], &CheckContext::now_with_defaults())
                .len(),
            2
        );
    }
}
