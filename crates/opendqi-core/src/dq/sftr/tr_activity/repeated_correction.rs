//! SFTR.TRA.REPEATED_CORRECTION — same UTI carries ≥3 CORR/MODI in
//! the same TAR batch.

use std::collections::BTreeMap;

use super::SftrTrActivityCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord, SftrTrStateRecord};

/// Check implementation.
pub struct SftrRepeatedCorrection;

const CHECK_ID: &str = "SFTR.TRA.REPEATED_CORRECTION";
const THRESHOLD: usize = 3;

impl SftrTrActivityCheck for SftrRepeatedCorrection {
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
        records: &[SftrRecord],
        _prior: &[SftrRecord],
        _tsr: Option<&[SftrTrStateRecord]>,
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
                    regime: Regime::Sftr,
                    severity: Severity::Warning,
                    dimension: DqDimension::Accuracy,
                    record_id: r.record_id.clone(),
                    uti: Some(uti.to_owned()),
                    field: Some("action_type".into()),
                    value: r.action_type.clone(),
                    message: format!(
                        "UTI {uti} has {n} correction rows in this SFTR TAR.",
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
    fn flags_three_corrections() {
        let recs = vec![
            SftrRecord {
                uti: Some("U1".into()),
                action_type: Some("CORR".into()),
                ..Default::default()
            };
            3
        ];
        assert_eq!(
            SftrRepeatedCorrection
                .run(&recs, &[], None, &CheckContext::now_with_defaults())
                .len(),
            3
        );
    }
}
