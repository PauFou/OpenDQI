//! `DQI_PAIRING_RATE` — weighted-average TR-side pairing rate
//! across all counterparties reported in an `auth.091`
//! reconciliation statistics file.
//!
//! - **Layer:** auth.091 (Reconciliation Statistics) projected
//!   onto [`crate::ReconStatsRecord`] — one record per
//!   counterparty pair.
//! - **Denominator:** counterparty records with a `pairing_rate`
//!   populated (the auth.091 parser derives it from cohort
//!   counts).
//! - **Numerator:** sum of `pairing_rate` values (weighted
//!   uniformly per record — equivalent to averaging when each
//!   counterparty carries one record).
//! - **Dimension:** consistency.
//!
//! **Honest scope** : v0.16 ships ONE pairing rate per
//! counterparty pair. The TCTN / POSI (trade-vs-position)
//! split that some supervisory DQ dashboards expose is **not**
//! captured by the current `auth.091` parser (cohorts are
//! collapsed into one `ReconStatsRecord` per LEI pair). A
//! stratum-aware refinement is v0.17+ candidate.
//!
//! **Status semantics** : low pairing is bad. Defaults
//! amber = 0.20 (= 80% paired) and red = 0.40 (= 60% paired)
//! mean the rate threshold is on the **unpaired share**,
//! i.e. `numerator = (1 - pairing_rate_weighted)`. Implementation
//! detail : we compute the "missed pairing" share so
//! [`rate_with_status`] applies the standard "higher = worse"
//! convention.

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, ReconStatsRecord, Regime};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_PAIRING_RATE";
const DESCRIPTION: &str =
    "Share of TR-side trade reports that are NOT paired with a counterparty submission, \
     averaged across counterparties (from auth.091 reconciliation statistics). Lower = better. \
     v0.16 ships one combined pairing rate per counterparty pair ; TCTN/POSI stratum split = v0.17+.";

/// Compute `DQI_PAIRING_RATE`.
pub fn compute_dqi_pairing_rate(
    recon_stats: &[ReconStatsRecord],
    thresholds: &Thresholds,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    let pair = resolve_threshold(thresholds, INDICATOR_ID);

    let mut denominator: u64 = 0;
    let mut numerator_unpaired_pct_sum: f64 = 0.0;
    let mut offenders: Vec<DqiEvidence> = Vec::new();

    for r in recon_stats {
        let Some(rate) = r.pairing_rate else {
            continue;
        };
        let rate_f64 = rate.to_string().parse::<f64>().unwrap_or(0.0);
        let unpaired_pct = 1.0 - rate_f64;
        denominator += 1;
        numerator_unpaired_pct_sum += unpaired_pct;

        // Evidence: counterparties with high unpaired share (worst offenders).
        if unpaired_pct > pair.amber {
            offenders.push(DqiEvidence {
                indicator_id: INDICATOR_ID.into(),
                uti: r.counterparty_lei.clone().unwrap_or_default(),
                counterparty: r.counterparty_lei.clone(),
                asset_class: None,
                source_file: r.source_file.clone(),
                observed_value: Some(format!("{:.4}", unpaired_pct)),
                explanation: format!(
                    "counterparty pairing rate = {:.4} (unpaired share {:.2}%)",
                    rate_f64,
                    unpaired_pct * 100.0
                ),
            });
        }
    }

    // Worst (highest unpaired share) first.
    offenders.sort_by(|a, b| {
        let av = a
            .observed_value
            .as_deref()
            .unwrap_or("0")
            .parse::<f64>()
            .unwrap_or(0.0);
        let bv = b
            .observed_value
            .as_deref()
            .unwrap_or("0")
            .parse::<f64>()
            .unwrap_or(0.0);
        bv.partial_cmp(&av).unwrap_or(std::cmp::Ordering::Equal)
    });
    offenders.truncate(EVIDENCE_TOP_N);

    // Rate = average unpaired share across counterparties.
    // Encode as numerator * 10_000 / denominator * 10_000 to fit the
    // rate_with_status u64 contract without precision loss.
    let scaled_num = (numerator_unpaired_pct_sum * 1_000_000.0) as u64;
    let scaled_denom = denominator.saturating_mul(1_000_000);
    let (rate, status) = rate_with_status(scaled_num, scaled_denom, &pair);

    let indicator = DqiIndicator {
        indicator_id: INDICATOR_ID.into(),
        regime: Regime::Emir,
        dimension: DqDimension::Consistency,
        table_scope: "auth.091".into(),
        numerator: scaled_num.min(scaled_denom),
        denominator: scaled_denom,
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
    use rust_decimal::Decimal;
    use std::str::FromStr;

    fn rec(lei: &str, rate: &str) -> ReconStatsRecord {
        ReconStatsRecord {
            counterparty_lei: Some(lei.into()),
            pairing_rate: Some(Decimal::from_str(rate).unwrap()),
            ..Default::default()
        }
    }

    #[test]
    fn empty_input_is_not_applicable() {
        let (ind, _) =
            compute_dqi_pairing_rate(&[], &Thresholds::default(), MappingPresence::default());
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn all_perfect_pairing_is_green() {
        let recs = vec![rec("LEI1", "1.0"), rec("LEI2", "1.0")];
        let (ind, _) =
            compute_dqi_pairing_rate(&recs, &Thresholds::default(), MappingPresence::default());
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn low_pairing_rate_fires_red() {
        // 2 counterparties, both at 50% pairing → 50% unpaired
        // → above default amber 0.05 / red 0.20 → red.
        let recs = vec![rec("LEI1", "0.5"), rec("LEI2", "0.5")];
        let (ind, ev) =
            compute_dqi_pairing_rate(&recs, &Thresholds::default(), MappingPresence::default());
        assert_eq!(ind.status, DqiStatus::Red);
        assert!(!ev.is_empty(), "evidence should list the offenders");
    }
}
