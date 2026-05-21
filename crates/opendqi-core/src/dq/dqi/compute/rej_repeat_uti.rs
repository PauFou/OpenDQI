//! `DQI_REJ_REPEAT_UTI` — share of distinct rejected UTIs that
//! were rejected ≥ 2 times within the feedback batch.
//!
//! Signals chronic rejections: a UTI failing the same (or
//! different) rule across multiple submission attempts is far
//! more painful operationally than a one-off rejection.
//!
//! - **Layer:** Feedback.
//! - **Denominator:** distinct UTIs with at least one
//!   `feedback_type == Rejected` row.
//! - **Numerator:** distinct UTIs rejected ≥ 2 times.
//! - **Dimension:** accuracy.

use std::collections::HashMap;

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, FeedbackRecord, FeedbackType, Regime};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_REJ_REPEAT_UTI";
const DESCRIPTION: &str = "Distinct UTIs rejected by the TR ≥ 2 times within the feedback batch \
— chronic rejection candidates.";

/// Compute `DQI_REJ_REPEAT_UTI`.
pub fn compute_dqi_rej_repeat_uti(
    feedback: &[FeedbackRecord],
    thresholds: &Thresholds,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    // UTI → (count, last_source_file).
    let mut counts: HashMap<String, (u64, Option<String>)> = HashMap::new();
    for f in feedback {
        if !matches!(f.feedback_type, FeedbackType::Rejected) {
            continue;
        }
        let Some(uti) = f.uti.as_ref() else { continue };
        let entry = counts
            .entry(uti.clone())
            .or_insert((0, f.source_file.clone()));
        entry.0 += 1;
        // keep the most recent source for evidence
        if f.source_file.is_some() {
            entry.1 = f.source_file.clone();
        }
    }

    let denominator: u64 = counts.len() as u64;
    let mut repeat_utis: Vec<(String, u64, Option<String>)> = counts
        .into_iter()
        .filter(|(_, (c, _))| *c >= 2)
        .map(|(u, (c, s))| (u, c, s))
        .collect();
    let numerator: u64 = repeat_utis.len() as u64;

    // Worst (most-rejected) first, then alphabetic on UTI for ties.
    repeat_utis.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    repeat_utis.truncate(EVIDENCE_TOP_N);

    let offenders: Vec<DqiEvidence> = repeat_utis
        .into_iter()
        .map(|(uti, count, source)| DqiEvidence {
            indicator_id: INDICATOR_ID.into(),
            uti,
            counterparty: None,
            asset_class: None,
            source_file: source,
            observed_value: Some(count.to_string()),
            explanation: format!("UTI rejected {count} times within this feedback batch"),
        })
        .collect();

    let pair = resolve_threshold(thresholds, INDICATOR_ID);
    let (rate, status) = rate_with_status(numerator, denominator, &pair);
    let indicator = DqiIndicator {
        indicator_id: INDICATOR_ID.into(),
        regime: Regime::Emir,
        dimension: DqDimension::Accuracy,
        table_scope: "Feedback".into(),
        numerator,
        denominator,
        rate,
        threshold_amber: Some(pair.amber),
        threshold_red: Some(pair.red),
        status,
        description: DESCRIPTION.into(),
    };
    (indicator, offenders)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dq::dqi::DqiStatus;

    fn rej(uti: &str) -> FeedbackRecord {
        FeedbackRecord {
            uti: Some(uti.into()),
            feedback_type: FeedbackType::Rejected,
            ..Default::default()
        }
    }

    #[test]
    fn empty_feedback_is_not_applicable() {
        let (ind, _) =
            compute_dqi_rej_repeat_uti(&[], &Thresholds::default(), MappingPresence::default());
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn single_rejections_yield_green() {
        let feedback = vec![rej("U1"), rej("U2"), rej("U3")];
        let (ind, _) = compute_dqi_rej_repeat_uti(
            &feedback,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 3);
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn repeat_utis_count_and_evidence_sort_by_count() {
        // U1 ×3, U2 ×2, U3 ×1 → 2 repeat over 3 distinct → 66.7 %
        let feedback = vec![
            rej("U1"),
            rej("U1"),
            rej("U1"),
            rej("U2"),
            rej("U2"),
            rej("U3"),
        ];
        let (ind, ev) = compute_dqi_rej_repeat_uti(
            &feedback,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 3);
        assert_eq!(ind.numerator, 2);
        assert_eq!(ev.len(), 2);
        assert_eq!(ev[0].uti, "U1");
        assert_eq!(ev[0].observed_value, Some("3".into()));
        assert_eq!(ev[1].uti, "U2");
    }

    #[test]
    fn non_rejected_feedback_excluded() {
        let mut feedback = vec![rej("U1"), rej("U1")];
        feedback.push(FeedbackRecord {
            uti: Some("U2".into()),
            feedback_type: FeedbackType::Missing,
            ..Default::default()
        });
        let (ind, _) = compute_dqi_rej_repeat_uti(
            &feedback,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        // U2 with Missing not counted, U1 is the only distinct rejected UTI
        assert_eq!(ind.denominator, 1);
        assert_eq!(ind.numerator, 1);
    }
}
