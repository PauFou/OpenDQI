//! `DQI_PAIRING_RATE_SFTR` — share of SFTR per-trade
//! reconciliation records (`auth.080` `RcncltnRpt`) whose
//! `pairing_status` is `UNPAIRED` (or equivalent), among those
//! with a `pairing_status` populated.
//!
//! - **Layer:** auth.080 SFTR Reconciliation Status Advice →
//!   [`crate::ReconciliationRecord`] filtered by
//!   `regime == Sftr` (defensive).
//! - **Denominator:** SFTR records with `pairing_status` set.
//! - **Numerator:** SFTR records where `pairing_status`
//!   (case-insensitive) is one of `UNPAIRED`, `NOT_PAIRED`,
//!   `UNPAIR` — tolerated short codes seen in real auth.080
//!   feeds.
//! - **Dimension:** consistency.
//!
//! Distinct from [`super::unpaired_trades_rate_sftr`] :
//! - `DQI_PAIRING_RATE_SFTR` uses `pairing_status set` as
//!   denominator (excludes records with no status).
//! - `DQI_UNPAIRED_TRADES_RATE_SFTR` uses **total records**
//!   as denominator (includes None-status records as
//!   non-unpaired).
//!
//! The two rates differ when a non-trivial subset of records
//! carries no `pairing_status` — operational diagnostic
//! signal.

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, ReconciliationRecord, Regime};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_PAIRING_RATE_SFTR";
const DESCRIPTION: &str = "Share of SFTR reconciliation records flagged UNPAIRED among those \
with a pairing_status populated (from auth.080 SFTR Reconciliation Status Advice). \
Complementary to DQI_UNPAIRED_TRADES_RATE_SFTR which uses total records as denominator.";

fn is_unpaired(status: Option<&str>) -> bool {
    status
        .map(|s| s.trim().to_ascii_uppercase())
        .map(|s| s.starts_with("UNPAIR") || s == "NOT_PAIRED" || s == "UNPR")
        .unwrap_or(false)
}

/// Compute `DQI_PAIRING_RATE_SFTR`.
pub fn compute_dqi_pairing_rate_sftr(
    recon_records: &[ReconciliationRecord],
    thresholds: &Thresholds,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    let mut denominator: u64 = 0;
    let mut numerator: u64 = 0;
    let mut offenders: Vec<DqiEvidence> = Vec::new();

    for r in recon_records {
        if r.regime != Regime::Sftr {
            continue;
        }
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
            explanation: "TR flagged this SFTR trade as UNPAIRED with the counterparty".into(),
        });
    }

    offenders.sort_by(|a, b| {
        a.source_file
            .cmp(&b.source_file)
            .then_with(|| a.counterparty.cmp(&b.counterparty))
            .then_with(|| a.uti.cmp(&b.uti))
    });
    offenders.truncate(EVIDENCE_TOP_N);

    let pair = resolve_threshold(thresholds, INDICATOR_ID);
    let (rate, status) = rate_with_status(numerator, denominator, &pair);
    let indicator = DqiIndicator {
        indicator_id: INDICATOR_ID.into(),
        regime: Regime::Sftr,
        dimension: DqDimension::Consistency,
        table_scope: "auth.080".into(),
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

    fn rec(uti: &str, regime: Regime, status: Option<&str>) -> ReconciliationRecord {
        ReconciliationRecord {
            uti: Some(uti.into()),
            regime,
            pairing_status: status.map(|s| s.into()),
            ..Default::default()
        }
    }

    #[test]
    fn empty_input_is_not_applicable() {
        let (ind, _) =
            compute_dqi_pairing_rate_sftr(&[], &Thresholds::default(), MappingPresence::default());
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn emir_records_ignored_by_regime_filter() {
        // Defensive : if a mixed EMIR+SFTR slice reaches us
        // (caller bug), the EMIR ReconciliationRecords are
        // filtered out so the rate computation stays SFTR-only.
        let recs = vec![
            rec("E1", Regime::Emir, Some("UNPAIRED")),
            rec("E2", Regime::Emir, Some("UNPAIRED")),
        ];
        let (ind, _) = compute_dqi_pairing_rate_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 0);
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn no_status_excluded_from_denominator() {
        let recs = vec![rec("S1", Regime::Sftr, None)];
        let (ind, _) = compute_dqi_pairing_rate_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 0);
    }

    #[test]
    fn all_paired_is_green() {
        let recs = vec![
            rec("S1", Regime::Sftr, Some("PAIRED")),
            rec("S2", Regime::Sftr, Some("PAIRED")),
        ];
        let (ind, _) = compute_dqi_pairing_rate_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn high_unpaired_share_is_red_with_short_code() {
        let recs = vec![
            rec("S1", Regime::Sftr, Some("UNPAIRED")),
            rec("S2", Regime::Sftr, Some("UNPR")),
            rec("S3", Regime::Sftr, Some("NOT_PAIRED")),
            rec("S4", Regime::Sftr, Some("PAIRED")),
        ];
        let (ind, ev) = compute_dqi_pairing_rate_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 4);
        assert_eq!(ind.numerator, 3);
        assert_eq!(ind.status, DqiStatus::Red);
        assert_eq!(ev.len(), 3);
    }
}
