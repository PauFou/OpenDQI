//! Shared helper for per-criterion mismatch DQIs.
//!
//! Used by `notional_inconsistent.rs`,
//! `margin_inconsistent_pre_haircut.rs`, and
//! `margin_inconsistent_post_haircut.rs` — three DQIs that
//! share the same computation shape : filter
//! `ReconciliationRecord.mismatched_fields` on a token set
//! and emit (numerator, denominator, top-N evidence).

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator};
use crate::model::{DqDimension, ReconciliationRecord, Regime};
use crate::scoring::rate_with_status;
use crate::Thresholds;

/// Compute a per-criterion-mismatch DQI from per-tx auth.091
/// records, filtering `mismatched_fields` on `tokens` (any
/// match counts toward the numerator).
///
/// Crate-internal — callers are the 3 thin wrapper DQIs in
/// the same module.
pub(crate) fn compute(
    recon_records: &[ReconciliationRecord],
    indicator_id: &'static str,
    description: &'static str,
    tokens: &[&str],
    dimension: DqDimension,
    thresholds: &Thresholds,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    let mut denominator: u64 = 0;
    let mut numerator: u64 = 0;
    let mut offenders: Vec<DqiEvidence> = Vec::new();

    for r in recon_records {
        if r.pairing_status.is_none() {
            continue;
        }
        denominator += 1;
        // Any token in `tokens` present in `mismatched_fields` →
        // fire.
        let matching: Vec<&str> = r
            .mismatched_fields
            .iter()
            .filter(|f| tokens.contains(&f.as_str()))
            .map(|f| f.as_str())
            .collect();
        if matching.is_empty() {
            continue;
        }
        numerator += 1;
        offenders.push(DqiEvidence {
            indicator_id: indicator_id.into(),
            uti: r.uti.clone().unwrap_or_default(),
            counterparty: r.reporting_counterparty.clone(),
            asset_class: None,
            source_file: r.source_file.clone(),
            observed_value: Some(matching.join(",")),
            explanation: format!(
                "{} criterion(s) mismatched: {}",
                matching.len(),
                matching.join(", ")
            ),
        });
    }

    offenders.sort_by(|a, b| {
        a.source_file
            .cmp(&b.source_file)
            .then_with(|| a.uti.cmp(&b.uti))
    });
    offenders.truncate(EVIDENCE_TOP_N);

    let pair = resolve_threshold(thresholds, indicator_id);
    let (rate, status) = rate_with_status(numerator, denominator, &pair);
    let indicator = DqiIndicator {
        indicator_id: indicator_id.into(),
        regime: Regime::Emir,
        dimension,
        table_scope: "auth.091".into(),
        numerator,
        denominator,
        rate,
        threshold_amber: Some(pair.amber),
        threshold_red: Some(pair.red),
        status,
        description: description.into(),
    };
    (indicator, offenders)
}
