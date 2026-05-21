//! `DQI_REJ_RATE` — share of feedback records that are
//! rejections.
//!
//! - **Layer:** Feedback (`auth.092`).
//! - **Denominator:** total feedback records.
//! - **Numerator:** records with `feedback_type == Rejected`.
//! - **Dimension:** accuracy.
//!
//! Note (honest scope limit): the "true" denominator is the
//! count of submissions made, which `auth.092` alone does not
//! carry. v0.15 ships "total feedback rows" as the proxy
//! denominator — this still answers "of the things the TR has
//! something to say about, how many are rejections?", which is
//! the operationally useful question. Documented in
//! `docs/data-quality-pack.md`.

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, FeedbackRecord, FeedbackType, Regime};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_REJ_RATE";
const DESCRIPTION: &str = "Share of feedback records that are TR rejections. Denominator is \
total feedback rows (proxy for total submissions).";

/// Compute `DQI_REJ_RATE`.
pub fn compute_dqi_rej_rate(
    feedback: &[FeedbackRecord],
    thresholds: &Thresholds,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    let mut denominator: u64 = 0;
    let mut numerator: u64 = 0;
    let mut offenders: Vec<DqiEvidence> = Vec::new();

    for f in feedback {
        denominator += 1;
        if !matches!(f.feedback_type, FeedbackType::Rejected) {
            continue;
        }
        numerator += 1;
        offenders.push(DqiEvidence {
            indicator_id: INDICATOR_ID.into(),
            uti: f.uti.clone().unwrap_or_default(),
            counterparty: None,
            asset_class: None,
            source_file: f.source_file.clone(),
            observed_value: f.reason_code.clone(),
            explanation: f
                .reason_description
                .clone()
                .unwrap_or_else(|| "TR rejected this submission".into()),
        });
    }

    offenders.sort_by(|a, b| {
        a.source_file
            .cmp(&b.source_file)
            .then_with(|| a.uti.cmp(&b.uti))
    });
    offenders.truncate(EVIDENCE_TOP_N);

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

    fn rejected(uti: &str) -> FeedbackRecord {
        FeedbackRecord {
            uti: Some(uti.into()),
            feedback_type: FeedbackType::Rejected,
            ..Default::default()
        }
    }

    fn missing(uti: &str) -> FeedbackRecord {
        FeedbackRecord {
            uti: Some(uti.into()),
            feedback_type: FeedbackType::Missing,
            ..Default::default()
        }
    }

    #[test]
    fn empty_feedback_is_not_applicable() {
        let (ind, _) =
            compute_dqi_rej_rate(&[], &Thresholds::default(), MappingPresence::default());
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn all_non_rejection_is_green() {
        let feedback = vec![missing("U1"), missing("U2")];
        let (ind, _) = compute_dqi_rej_rate(
            &feedback,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.denominator, 2);
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn rejection_breach() {
        // default thresholds: amber=0.01, red=0.05 → 10/100 = 0.10 → red
        let mut feedback = Vec::new();
        for i in 0..100 {
            feedback.push(if i < 10 {
                rejected(&format!("U{i}"))
            } else {
                missing(&format!("U{i}"))
            });
        }
        let (ind, ev) = compute_dqi_rej_rate(
            &feedback,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 10);
        assert_eq!(ind.denominator, 100);
        assert_eq!(ind.status, DqiStatus::Red);
        // Evidence truncated to top-20 (we only have 10 rejections)
        assert_eq!(ev.len(), 10);
    }
}
