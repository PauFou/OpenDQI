//! SFTR.TRA.SPIKE_TERM — ETRM proportion > 25%.

use super::SftrTrActivityCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord, SftrTrStateRecord};

/// Check implementation.
pub struct SftrSpikeTerm;

const CHECK_ID: &str = "SFTR.TRA.SPIKE_TERM";
const THRESHOLD_PCT: f64 = 25.0;

impl SftrTrActivityCheck for SftrSpikeTerm {
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
                    .map(|a| a.eq_ignore_ascii_case("ETRM") || a.eq_ignore_ascii_case("TERM"))
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
            severity: Severity::High,
            dimension: DqDimension::Accuracy,
            record_id: None,
            uti: None,
            field: Some("action_type".into()),
            value: Some(format!("{count:.0}/{total:.0}")),
            message: format!(
                "ETRM spike: {count:.0}/{total:.0} ({pct:.1}%) of the SFTR TAR are ETRM (threshold {THRESHOLD_PCT:.0}%)."
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
        let recs = vec![
            SftrRecord {
                action_type: Some("ETRM".into()),
                ..Default::default()
            };
            5
        ];
        assert_eq!(
            SftrSpikeTerm
                .run(&recs, &[], None, &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
}
