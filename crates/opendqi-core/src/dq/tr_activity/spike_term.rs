//! EMIR.TRA.SPIKE_TERM — proportion of `ETRM`/`TERM` actions in
//! the batch is above 25% (heuristic spike threshold).

use crate::dq::{CheckContext, TrActivityCheck};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity, TrStateRecord};

/// Check implementation.
pub struct EmirSpikeTerm;

const CHECK_ID: &str = "EMIR.TRA.SPIKE_TERM";
const THRESHOLD_PCT: f64 = 25.0;

fn is_termination(a: &str) -> bool {
    a.eq_ignore_ascii_case("ETRM") || a.eq_ignore_ascii_case("TERM")
}

impl TrActivityCheck for EmirSpikeTerm {
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
                    .map(is_termination)
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
            severity: Severity::High,
            dimension: DqDimension::Accuracy,
            record_id: None,
            uti: None,
            field: Some("action_type".into()),
            value: Some(format!("{count:.0}/{total:.0}")),
            message: format!(
                "Termination spike: {count:.0}/{total:.0} ({pct:.1}%) of the TAR batch are ETRM/TERM (threshold {THRESHOLD_PCT:.0}%)."
            ),
            source_file: records
                .first()
                .and_then(|r| r.source_file.clone()),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_termination_spike() {
        let recs = vec![
            EmirRecord {
                action_type: Some("ETRM".into()),
                ..Default::default()
            },
            EmirRecord {
                action_type: Some("ETRM".into()),
                ..Default::default()
            },
            EmirRecord {
                action_type: Some("NEWT".into()),
                ..Default::default()
            },
        ];
        assert_eq!(
            EmirSpikeTerm
                .run(&recs, &[], None, &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }

    #[test]
    fn ignores_normal_distribution() {
        let recs = vec![
            EmirRecord {
                action_type: Some("ETRM".into()),
                ..Default::default()
            },
            EmirRecord {
                action_type: Some("NEWT".into()),
                ..Default::default()
            },
            EmirRecord {
                action_type: Some("NEWT".into()),
                ..Default::default()
            },
            EmirRecord {
                action_type: Some("NEWT".into()),
                ..Default::default()
            },
            EmirRecord {
                action_type: Some("MODI".into()),
                ..Default::default()
            },
        ];
        assert!(EmirSpikeTerm
            .run(&recs, &[], None, &CheckContext::now_with_defaults())
            .is_empty());
    }
}
