//! EMIR.TRA.SPIKE_MODI — proportion of `MODI` actions in the batch
//! is above 40% (heuristic spike threshold for over-correction).

use crate::dq::{CheckContext, TrActivityCheck};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity, TrStateRecord};

/// Check implementation.
pub struct EmirSpikeModi;

const CHECK_ID: &str = "EMIR.TRA.SPIKE_MODI";
const THRESHOLD_PCT: f64 = 40.0;

impl TrActivityCheck for EmirSpikeModi {
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
            regime: Regime::Emir,
            severity: Severity::Warning,
            dimension: DqDimension::Accuracy,
            record_id: None,
            uti: None,
            field: Some("action_type".into()),
            value: Some(format!("{count:.0}/{total:.0}")),
            message: format!(
                "MODI spike: {count:.0}/{total:.0} ({pct:.1}%) of the TAR batch are MODI (threshold {THRESHOLD_PCT:.0}%)."
            ),
            source_file: records.first().and_then(|r| r.source_file.clone()),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_modi_spike() {
        let recs = (0..5)
            .map(|_| EmirRecord {
                action_type: Some("MODI".into()),
                ..Default::default()
            })
            .chain((0..3).map(|_| EmirRecord {
                action_type: Some("NEWT".into()),
                ..Default::default()
            }))
            .collect::<Vec<_>>();
        // 5/8 = 62.5% MODI > 40% → flagged
        assert_eq!(
            EmirSpikeModi
                .run(&recs, &[], None, &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }

    #[test]
    fn ignores_normal_modi_share() {
        let recs = (0..2)
            .map(|_| EmirRecord {
                action_type: Some("MODI".into()),
                ..Default::default()
            })
            .chain((0..8).map(|_| EmirRecord {
                action_type: Some("NEWT".into()),
                ..Default::default()
            }))
            .collect::<Vec<_>>();
        // 2/10 = 20% MODI → not flagged
        assert!(EmirSpikeModi
            .run(&recs, &[], None, &CheckContext::now_with_defaults())
            .is_empty());
    }
}
