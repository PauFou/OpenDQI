//! SFTR.TRA.DUPLICATE_NEWT_IN_BATCH — same UTI NEWT'd twice in the
//! same TAR batch.

use std::collections::BTreeMap;

use super::SftrTrActivityCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord, SftrTrStateRecord};

/// Check implementation.
pub struct SftrDuplicateNewtInBatch;

const CHECK_ID: &str = "SFTR.TRA.DUPLICATE_NEWT_IN_BATCH";

impl SftrTrActivityCheck for SftrDuplicateNewtInBatch {
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
        records: &[SftrRecord],
        _prior: &[SftrRecord],
        _tsr: Option<&[SftrTrStateRecord]>,
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        let mut by_uti: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
        for (idx, r) in records.iter().enumerate() {
            let is_newt = r
                .action_type
                .as_deref()
                .map(|a| a.eq_ignore_ascii_case("NEWT"))
                .unwrap_or(false);
            if !is_newt {
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
                    field: Some("action_type".into()),
                    value: Some("NEWT".into()),
                    message: format!(
                        "UTI {uti} carries {n} NEWT rows in the same SFTR TAR.",
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
            SftrRecord {
                uti: Some("U1".into()),
                action_type: Some("NEWT".into()),
                ..Default::default()
            };
            2
        ];
        assert_eq!(
            SftrDuplicateNewtInBatch
                .run(&recs, &[], None, &CheckContext::now_with_defaults())
                .len(),
            2
        );
    }
}
