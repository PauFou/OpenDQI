//! SFTR.TRA.SPIKE_MODI — MODI proportion > 40%.

use super::SftrTrActivityCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord, SftrTrStateRecord};

/// Check implementation.
pub struct SftrSpikeModi;

const CHECK_ID: &str = "SFTR.TRA.SPIKE_MODI";
const THRESHOLD_PCT: f64 = 40.0;

impl SftrTrActivityCheck for SftrSpikeModi {
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
        if records.is_empty() {
            return Vec::new();
        }
        let total = records.len() as f64;
        let count = records
            .iter()
            .filter(|r| {
                r.action_type
                    .as_deref()
                    .map(|a| a.eq_ignore_ascii_case("MODI"))
                    .unwrap_or(false)
            })
            .count() as f64;
        let pct = (count / total) * 100.0;
        if pct <= THRESHOLD_PCT {
            return Vec::new();
        }
        vec![DqIssue {
            check_id: CHECK_ID.into(),
            regime: Regime::Sftr,
            severity: Severity::Warning,
            dimension: DqDimension::Accuracy,
            record_id: None,
            uti: None,
            field: Some("action_type".into()),
            value: Some(format!("{count:.0}/{total:.0}")),
            message: format!(
                "MODI spike: {count:.0}/{total:.0} ({pct:.1}%) of the SFTR TAR are MODI (threshold {THRESHOLD_PCT:.0}%)."
            ),
            source_file: records.first().and_then(|r| r.source_file.clone()),
            evidence: Vec::new(),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flags_spike() {
        let recs = (0..6)
            .map(|_| SftrRecord {
                action_type: Some("MODI".into()),
                ..Default::default()
            })
            .chain((0..4).map(|_| SftrRecord {
                action_type: Some("NEWT".into()),
                ..Default::default()
            }))
            .collect::<Vec<_>>();
        assert_eq!(
            SftrSpikeModi
                .run(&recs, &[], None, &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
}
