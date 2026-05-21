//! `DQI_RECONCILIATION_RATE` — weighted-average TR-side
//! reconciliation rate (share of paired trades whose fields
//! reconcile across counterparties) from `auth.091`.
//!
//! - **Layer:** auth.091 → [`crate::ReconStatsRecord`].
//! - **Denominator:** counterparty records with `recon_rate`
//!   populated.
//! - **Numerator:** sum of `(1 - recon_rate)` per counterparty —
//!   the **un**-reconciled share, so higher = worse (matches the
//!   standard [`rate_with_status`] convention).
//! - **Dimension:** consistency.
//!
//! Mirror shape of `DQI_PAIRING_RATE` ; same caveats apply
//! (one rate per counterparty pair, no per-criterion split —
//! the per-criterion DQIs are
//! `DQI_NOTIONAL_INCONSISTENT` / `DQI_MARGIN_INCONSISTENT_*`).

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, ReconStatsRecord, Regime};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_RECONCILIATION_RATE";
const DESCRIPTION: &str =
    "Share of paired trades that do NOT reconcile across counterparties (from auth.091). \
     Lower = better. Per-criterion breakdowns are surfaced by DQI_NOTIONAL_INCONSISTENT and \
     DQI_MARGIN_INCONSISTENT_PRE/POST_HAIRCUT.";

/// Compute `DQI_RECONCILIATION_RATE`.
pub fn compute_dqi_reconciliation_rate(
    recon_stats: &[ReconStatsRecord],
    thresholds: &Thresholds,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    let pair = resolve_threshold(thresholds, INDICATOR_ID);

    let mut denominator: u64 = 0;
    let mut numerator_unrec_pct_sum: f64 = 0.0;
    let mut offenders: Vec<DqiEvidence> = Vec::new();

    for r in recon_stats {
        let Some(rate) = r.recon_rate else {
            continue;
        };
        let rate_f64 = rate.to_string().parse::<f64>().unwrap_or(0.0);
        let unrec_pct = 1.0 - rate_f64;
        denominator += 1;
        numerator_unrec_pct_sum += unrec_pct;

        if unrec_pct > pair.amber {
            offenders.push(DqiEvidence {
                indicator_id: INDICATOR_ID.into(),
                uti: r.counterparty_lei.clone().unwrap_or_default(),
                counterparty: r.counterparty_lei.clone(),
                asset_class: None,
                source_file: r.source_file.clone(),
                observed_value: Some(format!("{:.4}", unrec_pct)),
                explanation: format!(
                    "counterparty reconciliation rate = {:.4} (unreconciled share {:.2}%)",
                    rate_f64,
                    unrec_pct * 100.0
                ),
            });
        }
    }

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

    let scaled_num = (numerator_unrec_pct_sum * 1_000_000.0) as u64;
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
            recon_rate: Some(Decimal::from_str(rate).unwrap()),
            ..Default::default()
        }
    }

    #[test]
    fn empty_input_is_not_applicable() {
        let (ind, _) = compute_dqi_reconciliation_rate(
            &[],
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn perfect_recon_is_green() {
        let recs = vec![rec("LEI1", "1.0"), rec("LEI2", "1.0")];
        let (ind, _) = compute_dqi_reconciliation_rate(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn low_recon_rate_fires_red() {
        let recs = vec![rec("LEI1", "0.6"), rec("LEI2", "0.7")];
        let (ind, _) = compute_dqi_reconciliation_rate(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.status, DqiStatus::Red);
    }
}
