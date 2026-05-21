//! `DQI_UNPAIRED_TRADES_RATE` — share of per-trade
//! reconciliation records (`auth.091` `RcncltnRpt`) whose
//! `pairing_status` is `UNPAIRED`.
//!
//! - **Layer:** auth.091 per-tx reconciliation records →
//!   [`crate::ReconciliationRecord`].
//! - **Denominator:** records with `pairing_status` populated.
//! - **Numerator:** records where `pairing_status` (case-
//!   insensitive) starts with `UNPAIR` (`UNPAIRED`/`UNPR`).
//! - **Dimension:** consistency.
//!
//! Complementary to `DQI_PAIRING_RATE` :
//! - `DQI_PAIRING_RATE` is the **counterparty-level** rate
//!   (averaged across LEI pairs from cohort totals)
//! - `DQI_UNPAIRED_TRADES_RATE` is the **per-trade** rate
//!   (counted from per-tx records)
//!
//! The two will diverge when the auth.091 file ships cohort
//! totals without per-tx detail (or vice-versa).

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, ReconciliationRecord, Regime};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_UNPAIRED_TRADES_RATE";
const DESCRIPTION: &str = "Share of per-trade reconciliation records with pairing_status=UNPAIRED \
(from auth.091 per-tx detail). Complementary to DQI_PAIRING_RATE which counts at the counterparty level.";

fn is_unpaired(status: Option<&str>) -> bool {
    status
        .map(|s| s.trim().to_ascii_uppercase())
        .map(|s| s.starts_with("UNPAIR") || s.starts_with("UNPR"))
        .unwrap_or(false)
}

/// Compute `DQI_UNPAIRED_TRADES_RATE`.
pub fn compute_dqi_unpaired_trades_rate(
    recon_records: &[ReconciliationRecord],
    thresholds: &Thresholds,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    let mut denominator: u64 = 0;
    let mut numerator: u64 = 0;
    let mut offenders: Vec<DqiEvidence> = Vec::new();

    for r in recon_records {
        if r.pairing_status.is_none() {
            continue;
        }
        denominator += 1;
        if !is_unpaired(r.pairing_status.as_deref()) {
            continue;
        }
        numerator += 1;
        offenders.push(DqiEvidence {
            indicator_id: INDICATOR_ID.into(),
            uti: r.uti.clone().unwrap_or_default(),
            counterparty: r.reporting_counterparty.clone(),
            asset_class: None,
            source_file: r.source_file.clone(),
            observed_value: r.pairing_status.clone(),
            explanation: "TR flagged this trade as UNPAIRED with the counterparty".into(),
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
        dimension: DqDimension::Consistency,
        table_scope: "auth.091".into(),
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

    fn rec(uti: &str, status: Option<&str>) -> ReconciliationRecord {
        ReconciliationRecord {
            uti: Some(uti.into()),
            pairing_status: status.map(|s| s.into()),
            ..Default::default()
        }
    }

    #[test]
    fn empty_input_is_not_applicable() {
        let (ind, _) = compute_dqi_unpaired_trades_rate(
            &[],
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn no_status_excluded_from_denominator() {
        let recs = vec![rec("U1", None)];
        let (ind, _) = compute_dqi_unpaired_trades_rate(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 0);
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn all_paired_is_green() {
        let recs = vec![rec("U1", Some("PAIRED")), rec("U2", Some("PAIRED"))];
        let (ind, _) = compute_dqi_unpaired_trades_rate(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn high_unpaired_share_is_red() {
        let recs = vec![
            rec("U1", Some("UNPAIRED")),
            rec("U2", Some("UNPAIRED")),
            rec("U3", Some("PAIRED")),
        ];
        let (ind, ev) = compute_dqi_unpaired_trades_rate(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 3);
        assert_eq!(ind.numerator, 2);
        assert_eq!(ind.status, DqiStatus::Red);
        assert_eq!(ev.len(), 2);
    }

    #[test]
    fn unpr_short_code_also_counts() {
        // auth.091 uses Pairg=UNPR ; the projected status is
        // typically "UNPAIRED" but tolerate the short code too.
        let recs = vec![rec("U1", Some("UNPR"))];
        let (ind, _) = compute_dqi_unpaired_trades_rate(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 1);
    }
}
