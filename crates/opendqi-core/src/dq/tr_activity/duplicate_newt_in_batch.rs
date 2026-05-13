//! EMIR.TRA.DUPLICATE_NEWT_IN_BATCH — same UTI carries 2 or more
//! `NEWT` action rows in the same TAR batch — never legitimate,
//! always a defect.

use std::collections::BTreeMap;

use crate::dq::{CheckContext, TrActivityCheck};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity, TrStateRecord};

/// Check implementation.
pub struct EmirDuplicateNewtInBatch;

const CHECK_ID: &str = "EMIR.TRA.DUPLICATE_NEWT_IN_BATCH";

impl TrActivityCheck for EmirDuplicateNewtInBatch {
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
        records: &[EmirRecord],
        _prior: &[EmirRecord],
        _tsr: Option<&[TrStateRecord]>,
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
                    regime: Regime::Emir,
                    severity: Severity::Critical,
                    dimension: DqDimension::Uniqueness,
                    record_id: r.record_id.clone(),
                    uti: Some(uti.to_owned()),
                    field: Some("action_type".into()),
                    value: Some("NEWT".into()),
                    message: format!(
                        "UTI {uti} carries {n} NEWT rows in the same TAR batch.",
                        n = indices.len()
                    ),
                    source_file: r.source_file.clone(),
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
    fn flags_duplicate_newt() {
        let recs = vec![
            EmirRecord {
                uti: Some("U1".into()),
                action_type: Some("NEWT".into()),
                ..Default::default()
            },
            EmirRecord {
                uti: Some("U1".into()),
                action_type: Some("NEWT".into()),
                ..Default::default()
            },
        ];
        assert_eq!(
            EmirDuplicateNewtInBatch
                .run(&recs, &[], None, &CheckContext::now_with_defaults())
                .len(),
            2
        );
    }

    #[test]
    fn singleton_is_ok() {
        let recs = vec![EmirRecord {
            uti: Some("U1".into()),
            action_type: Some("NEWT".into()),
            ..Default::default()
        }];
        assert!(EmirDuplicateNewtInBatch
            .run(&recs, &[], None, &CheckContext::now_with_defaults())
            .is_empty());
    }
}
