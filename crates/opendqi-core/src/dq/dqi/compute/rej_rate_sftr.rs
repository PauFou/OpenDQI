//! `DQI_REJ_RATE_SFTR` — share of SFTR submissions rejected by
//! the TR, computed from the aggregate `auth.084` (Transaction
//! Status Advice) statistics.
//!
//! - **Layer:** SFTR Transaction Status Advice (`auth.084`).
//! - **Denominator:** sum of `total_reports` across every
//!   `SftrTrStatusAdviceRecord` in the input slice. Multiple
//!   records (one per file) aggregate naturally; a single file
//!   typically carries one record.
//! - **Numerator:** sum of `total_reports_rejected` across the
//!   same slice.
//! - **Dimension:** accuracy.
//!
//! Mirror of EMIR `DQI_REJ_RATE` (which operates on per-record
//! feedback `auth.092`). The SFTR side ships aggregate stats
//! at the XSD level so the computer just sums totals — no
//! per-record iteration needed.
//!
//! Evidence: top-N per-error-code rows aggregated across the
//! slice, sorted by count descending. Each row carries the
//! validation rule code in `uti` (the canonical "primary
//! record key" column in the v1.0 Arrow schema, semantic
//! adapts per indicator) and the count in `observed_value`.

use std::collections::BTreeMap;

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, Regime, SftrTrStatusAdviceRecord};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_REJ_RATE_SFTR";
const DESCRIPTION: &str = "Share of SFTR submissions rejected by the TR, computed from the \
aggregate auth.084 (Transaction Status Advice) statistics: sum(total_reports_rejected) / \
sum(total_reports) across every input record. Mirror of EMIR DQI_REJ_RATE.";

/// Compute `DQI_REJ_RATE_SFTR`.
pub fn compute_dqi_rej_rate_sftr(
    advice: &[SftrTrStatusAdviceRecord],
    thresholds: &Thresholds,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    let mut denominator: u64 = 0;
    let mut numerator: u64 = 0;
    // Aggregate the per-error breakdown across the whole slice
    // so the evidence rows reflect the *batch* error distribution
    // not the first file's.
    let mut per_err_total: BTreeMap<String, u64> = BTreeMap::new();
    // First non-None source_file we see — used to label the
    // evidence rows.
    let mut representative_source: Option<String> = None;

    for r in advice {
        denominator = denominator.saturating_add(r.total_reports.unwrap_or(0));
        numerator = numerator.saturating_add(r.total_reports_rejected.unwrap_or(0));
        for (code, n) in &r.rejected_reports_per_error {
            let entry = per_err_total.entry(code.clone()).or_insert(0);
            *entry = entry.saturating_add(*n);
        }
        if representative_source.is_none() && r.source_file.is_some() {
            representative_source = r.source_file.clone();
        }
    }

    // Evidence: top-N error codes by count, descending.
    let mut ranked: Vec<(String, u64)> = per_err_total.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let offenders: Vec<DqiEvidence> = ranked
        .into_iter()
        .take(EVIDENCE_TOP_N)
        .map(|(code, count)| DqiEvidence {
            indicator_id: INDICATOR_ID.into(),
            uti: code.clone(),
            counterparty: None,
            asset_class: None,
            source_file: representative_source.clone(),
            observed_value: Some(count.to_string()),
            explanation: format!(
                "validation rule {code} accounts for {count} rejected report(s) in the batch"
            ),
        })
        .collect();

    let pair = resolve_threshold(thresholds, INDICATOR_ID);
    let (rate, status) = rate_with_status(numerator, denominator, &pair);
    let indicator = DqiIndicator {
        indicator_id: INDICATOR_ID.into(),
        regime: Regime::Sftr,
        dimension: DqDimension::Accuracy,
        table_scope: "SFTR-TSA".into(),
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

    fn rec(total: u64, rejected: u64) -> SftrTrStatusAdviceRecord {
        SftrTrStatusAdviceRecord {
            total_reports: Some(total),
            total_reports_accepted: Some(total.saturating_sub(rejected)),
            total_reports_rejected: Some(rejected),
            ..Default::default()
        }
    }

    #[test]
    fn empty_is_not_applicable() {
        let (ind, _) =
            compute_dqi_rej_rate_sftr(&[], &Thresholds::default(), MappingPresence::default());
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn zero_rejections_is_green() {
        let (ind, _) = compute_dqi_rej_rate_sftr(
            &[rec(1000, 0)],
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.denominator, 1000);
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn aggregates_totals_across_multiple_records() {
        // Two files of stats: 1000+500 reports, 50+10 rejected
        // → 60/1500 = 4.0 % rate.
        let (ind, _) = compute_dqi_rej_rate_sftr(
            &[rec(1000, 50), rec(500, 10)],
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 1500);
        assert_eq!(ind.numerator, 60);
        // 0.04 ≤ amber 0.05 → green by the inclusive-boundary rule.
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn high_rejection_rate_is_red() {
        // 30 % rejection → above the 20 % red boundary.
        let (ind, _) = compute_dqi_rej_rate_sftr(
            &[rec(100, 30)],
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.status, DqiStatus::Red);
    }

    #[test]
    fn per_error_breakdown_aggregates_and_sorts_descending() {
        let mut r1 = rec(100, 10);
        r1.rejected_reports_per_error.insert("VR-001".into(), 6);
        r1.rejected_reports_per_error.insert("VR-002".into(), 4);
        let mut r2 = rec(100, 5);
        r2.rejected_reports_per_error.insert("VR-001".into(), 3);
        r2.rejected_reports_per_error.insert("VR-099".into(), 2);
        let (_, ev) = compute_dqi_rej_rate_sftr(
            &[r1, r2],
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ev.len(), 3);
        // VR-001 = 6+3 = 9, VR-002 = 4, VR-099 = 2.
        // Sorted descending by count.
        assert_eq!(ev[0].uti, "VR-001");
        assert_eq!(ev[0].observed_value.as_deref(), Some("9"));
        assert_eq!(ev[1].uti, "VR-002");
        assert_eq!(ev[1].observed_value.as_deref(), Some("4"));
        assert_eq!(ev[2].uti, "VR-099");
    }

    #[test]
    fn missing_totals_are_treated_as_zero_no_panic() {
        let r = SftrTrStatusAdviceRecord {
            total_reports: None,
            total_reports_accepted: None,
            total_reports_rejected: None,
            ..Default::default()
        };
        let (ind, _) =
            compute_dqi_rej_rate_sftr(&[r], &Thresholds::default(), MappingPresence::default());
        // 0/0 → NotApplicable.
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }
}
