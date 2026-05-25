//! `DQI_MAR_EVENT_SPIKE_SFTR` — share of CP-pairs whose SFTR
//! MAR (auth.070) event count exceeds **mean + 2σ** vs the
//! observed-batch baseline.
//!
//! - **Layer:** SFTR MAR (`auth.070`).
//! - **Granularity:** per (reporting_counterparty,
//!   other_counterparty) pair.
//! - **Denominator:** number of distinct CP-pairs observed in
//!   the input batch. Records without both counterparties
//!   populated are excluded (they can't be grouped). When fewer
//!   than 4 distinct pairs are present the indicator returns
//!   `NotApplicable` (insufficient sample for a 2σ baseline).
//! - **Numerator:** number of CP-pairs whose event count is
//!   strictly greater than `mean(counts) + 2 * stddev(counts)`.
//! - **Dimension:** timeliness (operational anomaly / stress
//!   indicator).
//!
//! Rationale: a CP-pair that suddenly produces 10× the
//! normal-batch event rate is an operational anomaly (a
//! reporting backlog flush, an upstream replay storm, a margin-
//! call cascade). The N-sigma test is a baseline-free way to
//! surface these without per-pair history — the batch itself
//! is the baseline. Threshold pair calibrated against the
//! existing reconciliation tolerances (5 % amber / 20 % red):
//! more than one CP-pair in twenty being a spike is unusual.

use std::collections::HashMap;

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, DqiStatus, MappingPresence};
use crate::model::{DqDimension, Regime, SftrMarginActivityRecord};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_MAR_EVENT_SPIKE_SFTR";
const DESCRIPTION: &str = "Share of CP-pairs whose SFTR MAR (auth.070) event count exceeds \
mean + 2σ vs the batch baseline. Surfaces operational anomalies (reporting backlog flush, \
replay storm, margin-call cascade) without needing per-pair history. Requires ≥ 4 distinct \
CP-pairs to be statistically meaningful (returns NotApplicable below that).";

/// Minimum distinct CP-pairs required for the 2σ baseline to
/// be meaningful. Below this the indicator returns
/// `NotApplicable` rather than fabricate a spurious rate.
const MIN_PAIRS_FOR_STATS: usize = 4;

/// Per-CP-pair accumulator: (event_count, source_file, portfolio_label).
/// Aliased so the per-pair `HashMap` value type stays clippy-friendly.
type PairStats = (u64, Option<String>, Option<String>);

/// Compute `DQI_MAR_EVENT_SPIKE_SFTR`.
pub fn compute_dqi_mar_event_spike_sftr(
    mar: &[SftrMarginActivityRecord],
    thresholds: &Thresholds,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    // Group event counts per CP-pair.
    let mut counts: HashMap<(String, String), PairStats> = HashMap::new();
    for r in mar {
        let (Some(reporting), Some(other)) = (
            r.reporting_counterparty.as_deref(),
            r.other_counterparty.as_deref(),
        ) else {
            continue;
        };
        let entry = counts
            .entry((reporting.to_owned(), other.to_owned()))
            .or_insert((
                0,
                r.source_file.clone(),
                r.collateral_portfolio_code.clone(),
            ));
        entry.0 += 1;
        if entry.2.is_none() {
            entry.2 = r.collateral_portfolio_code.clone();
        }
    }
    let denominator = counts.len() as u64;

    // Sample-size guard: fewer than MIN_PAIRS_FOR_STATS distinct
    // pairs ⇒ no 2σ baseline ⇒ NotApplicable.
    if (counts.len()) < MIN_PAIRS_FOR_STATS {
        let pair = resolve_threshold(thresholds, INDICATOR_ID);
        return (
            DqiIndicator {
                indicator_id: INDICATOR_ID.into(),
                regime: Regime::Sftr,
                dimension: DqDimension::Timeliness,
                table_scope: "SFTR-MAR".into(),
                numerator: 0,
                denominator,
                rate: None,
                threshold_amber: Some(pair.amber),
                threshold_red: Some(pair.red),
                status: DqiStatus::NotApplicable,
                description: format!(
                    "{DESCRIPTION} Observed {denominator} CP-pair(s), need ≥ {MIN_PAIRS_FOR_STATS}."
                ),
            },
            Vec::new(),
        );
    }

    // Compute mean + stddev over the CP-pair event counts.
    let n = counts.len() as f64;
    let mean: f64 = counts.values().map(|(c, _, _)| *c as f64).sum::<f64>() / n;
    let variance: f64 = counts
        .values()
        .map(|(c, _, _)| {
            let d = *c as f64 - mean;
            d * d
        })
        .sum::<f64>()
        / n;
    let stddev = variance.sqrt();
    let cutoff = mean + 2.0 * stddev;

    // Identify spikes.
    let mut numerator: u64 = 0;
    let mut offenders: Vec<DqiEvidence> = Vec::new();
    for ((reporting, other), (count, source, portfolio)) in counts.iter() {
        if (*count as f64) <= cutoff {
            continue;
        }
        numerator += 1;
        let uti_label = portfolio
            .clone()
            .unwrap_or_else(|| format!("{reporting}↔{other}"));
        offenders.push(DqiEvidence {
            indicator_id: INDICATOR_ID.into(),
            uti: uti_label,
            counterparty: Some(reporting.clone()),
            asset_class: None,
            source_file: source.clone(),
            observed_value: Some(format!("{count} events")),
            explanation: format!(
                "CP-pair ({reporting}, {other}) reports {count} MAR events; \
                 batch baseline = mean {mean:.2} + 2σ {two_sigma:.2} (cutoff {cutoff:.2})",
                two_sigma = 2.0 * stddev
            ),
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
        regime: Regime::Sftr,
        dimension: DqDimension::Timeliness,
        table_scope: "SFTR-MAR".into(),
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

    fn rec(reporting: &str, other: &str, portfolio: &str) -> SftrMarginActivityRecord {
        SftrMarginActivityRecord {
            reporting_counterparty: Some(reporting.into()),
            other_counterparty: Some(other.into()),
            collateral_portfolio_code: Some(portfolio.into()),
            ..Default::default()
        }
    }

    #[test]
    fn empty_is_not_applicable_with_explanation() {
        let (ind, ev) = compute_dqi_mar_event_spike_sftr(
            &[],
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.status, DqiStatus::NotApplicable);
        assert_eq!(ind.denominator, 0);
        assert!(ev.is_empty());
        assert!(ind.description.contains("Observed 0 CP-pair"));
    }

    #[test]
    fn below_min_pairs_is_not_applicable() {
        // 3 distinct CP-pairs (3 < 4) ⇒ insufficient sample.
        let recs = vec![
            rec("A", "X", "P1"),
            rec("A", "Y", "P2"),
            rec("A", "Z", "P3"),
        ];
        let (ind, _) = compute_dqi_mar_event_spike_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.status, DqiStatus::NotApplicable);
        assert_eq!(ind.denominator, 3);
    }

    #[test]
    fn flat_distribution_no_spike() {
        // 5 CP-pairs, each with 2 events: stddev=0, no spike.
        let mut recs = Vec::new();
        for i in 0..5 {
            let other = format!("CP-{i}");
            recs.push(rec("A", &other, &format!("P-{i}-a")));
            recs.push(rec("A", &other, &format!("P-{i}-b")));
        }
        let (ind, _) = compute_dqi_mar_event_spike_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 5);
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn one_extreme_outlier_fires_as_spike() {
        // 9 baseline pairs at 1 event + 1 anomalous pair at 100
        // events. Numerical check:
        //   mean    = (9 + 100) / 10 = 10.9
        //   var     = (9 * (10.9-1)^2 + (100-10.9)^2) / 10 ≈ 882.1
        //   stddev  ≈ 29.7
        //   cutoff  ≈ 10.9 + 59.4 = 70.3
        //   outlier 100 > 70.3 ⇒ flagged.
        // n=10 keeps the outlier from dominating the stats so
        // hard it pulls itself below the 2σ line (the n=5 case).
        let mut recs = Vec::new();
        for i in 0..9 {
            let other = format!("CP-{i}");
            recs.push(rec("A", &other, &format!("P-{i}")));
        }
        for k in 0..100 {
            recs.push(rec("A", "CP-SPIKE", &format!("P-SPIKE-{k}")));
        }
        let (ind, ev) = compute_dqi_mar_event_spike_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 10);
        assert_eq!(ind.numerator, 1);
        assert_eq!(ev.len(), 1);
        assert!(
            ev[0].explanation.contains("CP-SPIKE"),
            "spike evidence should name the outlier pair: {}",
            ev[0].explanation
        );
    }

    #[test]
    fn records_missing_counterparty_excluded_from_grouping() {
        // 4 well-formed CP-pairs satisfy MIN_PAIRS_FOR_STATS.
        // The 5th record is missing other_counterparty → silently
        // dropped from the denominator (cannot be grouped).
        let mut recs = vec![
            rec("A", "X", "P1"),
            rec("A", "Y", "P2"),
            rec("A", "Z", "P3"),
            rec("A", "W", "P4"),
        ];
        let mut orphan = rec("A", "ignored", "P-ORPHAN");
        orphan.other_counterparty = None;
        recs.push(orphan);
        let (ind, _) = compute_dqi_mar_event_spike_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        // The orphan was excluded from grouping → 4 distinct pairs.
        assert_eq!(ind.denominator, 4);
    }
}
