//! EMIR.TRA.REPEATED_CORRECTION — same UTI carries 3 or more
//! `CORR` (or `MODI`) action rows in the same TAR batch — strong
//! signal of an upstream data-quality issue.

use std::collections::BTreeMap;

use crate::dq::{CheckContext, TrActivityCheck};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity, TrStateRecord};

/// Check implementation.
pub struct EmirRepeatedCorrection;

const CHECK_ID: &str = "EMIR.TRA.REPEATED_CORRECTION";
const THRESHOLD: usize = 3;

impl TrActivityCheck for EmirRepeatedCorrection {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
    }
    fn severity(&self) -> Severity {
        Severity::Warning
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
            let is_corr = r
                .action_type
                .as_deref()
                .map(|a| a.eq_ignore_ascii_case("CORR") || a.eq_ignore_ascii_case("MODI"))
                .unwrap_or(false);
            if !is_corr {
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
            if indices.len() < THRESHOLD {
                continue;
            }
            for &idx in &indices {
                let r = &records[idx];
                out.push(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Emir,
                    severity: Severity::Warning,
                    dimension: DqDimension::Accuracy,
                    record_id: r.record_id.clone(),
                    uti: Some(uti.to_owned()),
                    field: Some("action_type".into()),
                    value: r.action_type.clone(),
                    message: format!(
                        "UTI {uti} has {n} correction/modification rows in this TAR batch (threshold: {THRESHOLD}).",
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
    fn flags_three_corrections_for_same_uti() {
        let recs = vec![
            EmirRecord {
                uti: Some("U1".into()),
                action_type: Some("CORR".into()),
                ..Default::default()
            },
            EmirRecord {
                uti: Some("U1".into()),
                action_type: Some("MODI".into()),
                ..Default::default()
            },
            EmirRecord {
                uti: Some("U1".into()),
                action_type: Some("CORR".into()),
                ..Default::default()
            },
        ];
        let issues =
            EmirRepeatedCorrection.run(&recs, &[], None, &CheckContext::now_with_defaults());
        assert_eq!(issues.len(), 3);
    }

    #[test]
    fn ignores_two_corrections() {
        let recs = vec![
            EmirRecord {
                uti: Some("U1".into()),
                action_type: Some("CORR".into()),
                ..Default::default()
            },
            EmirRecord {
                uti: Some("U1".into()),
                action_type: Some("CORR".into()),
                ..Default::default()
            },
        ];
        assert!(EmirRepeatedCorrection
            .run(&recs, &[], None, &CheckContext::now_with_defaults())
            .is_empty());
    }
}
