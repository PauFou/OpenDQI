//! `DQI_VAL_MISSING` — share of outstanding TSR records that
//! have no valuation amount (or zero valuation, treated as
//! "no valuation reported" per ESMA DQ dashboards convention).
//!
//! - **Layer:** TSR (`auth.107`, modelled as
//!   [`crate::model::TrStateRecord`]).
//! - **Denominator:** outstanding TSR records — `status` is
//!   neither `MATURED` nor `TERMINATED` *and* `termination_date`
//!   is unset.
//! - **Numerator:** outstanding records where `valuation_amount`
//!   is `None` or equal to zero.
//! - **Dimension:** completeness.

use rust_decimal::Decimal;

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, Regime, TrStateRecord};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_VAL_MISSING";
const DESCRIPTION: &str = "Outstanding TSR records with no valuation amount \
(or a zero valuation, treated as not reported).";

fn is_outstanding(r: &TrStateRecord) -> bool {
    if r.termination_date.is_some() {
        return false;
    }
    match r.status.as_deref() {
        Some(s) => {
            let up = s.trim().to_ascii_uppercase();
            !(up.starts_with("MATUR") || up.starts_with("TERMIN"))
        }
        None => true, // unknown status — counted as still open (defensive)
    }
}

/// Compute `DQI_VAL_MISSING` on a TSR snapshot.
///
/// `_as_of` is reserved for future business-day-aware
/// refinements (currently unused — the check is purely
/// presence-based on the snapshot).
pub fn compute_dqi_val_missing(
    tsr: &[TrStateRecord],
    thresholds: &Thresholds,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    let mut denominator: u64 = 0;
    let mut numerator: u64 = 0;
    let mut offenders: Vec<DqiEvidence> = Vec::new();

    for r in tsr {
        if !is_outstanding(r) {
            continue;
        }
        denominator += 1;
        let missing = r
            .valuation_amount
            .as_ref()
            .map(|d| *d == Decimal::ZERO)
            .unwrap_or(true);
        if !missing {
            continue;
        }
        numerator += 1;
        // Sort key for top-20: record_id is stable per source,
        // so we order by `(source_file, record_id)` later; for now
        // just collect every offender and truncate after sort.
        let observed_value = r.valuation_amount.as_ref().map(|d| d.to_string());
        offenders.push(DqiEvidence {
            indicator_id: INDICATOR_ID.into(),
            uti: r.uti.clone().unwrap_or_default(),
            counterparty: r.reporting_counterparty.clone(),
            asset_class: None,
            source_file: r.source_file.clone(),
            observed_value,
            explanation: "valuation_amount is missing or zero on an outstanding trade".into(),
        });
    }

    // Deterministic top-20: sort by `(source_file, uti)` ascending
    // (matches the issues.csv comparator philosophy).
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
        dimension: DqDimension::Completeness,
        table_scope: "TSR".into(),
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
    use rust_decimal::Decimal;

    use super::*;
    use crate::dq::dqi::DqiStatus;
    use crate::Thresholds;

    fn rec(uti: &str, status: Option<&str>, val: Option<Decimal>) -> TrStateRecord {
        TrStateRecord {
            uti: Some(uti.into()),
            status: status.map(|s| s.into()),
            valuation_amount: val,
            ..Default::default()
        }
    }

    #[test]
    fn empty_input_is_not_applicable() {
        let (ind, ev) =
            compute_dqi_val_missing(&[], &Thresholds::default(), MappingPresence::default());
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.denominator, 0);
        assert_eq!(ind.status, DqiStatus::NotApplicable);
        assert_eq!(ind.rate, None);
        assert!(ev.is_empty());
    }

    #[test]
    fn matured_records_skipped_from_denominator() {
        let recs = vec![
            rec("U1", Some("MATURED"), None),
            rec("U2", Some("TERMINATED"), None),
        ];
        let (ind, _) =
            compute_dqi_val_missing(&recs, &Thresholds::default(), MappingPresence::default());
        assert_eq!(ind.denominator, 0, "MATURED + TERMINATED excluded");
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn all_present_valuations_is_green() {
        let recs = vec![
            rec("U1", Some("OUTSTANDING"), Some(Decimal::new(100, 0))),
            rec("U2", Some("OUTSTANDING"), Some(Decimal::new(200, 0))),
        ];
        let (ind, ev) =
            compute_dqi_val_missing(&recs, &Thresholds::default(), MappingPresence::default());
        assert_eq!(ind.denominator, 2);
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.rate, Some(0.0));
        assert_eq!(ind.status, DqiStatus::Green);
        assert!(ev.is_empty());
    }

    #[test]
    fn zero_valuation_counts_as_missing() {
        let recs = vec![rec("U1", Some("OUTSTANDING"), Some(Decimal::ZERO))];
        let (ind, ev) =
            compute_dqi_val_missing(&recs, &Thresholds::default(), MappingPresence::default());
        assert_eq!(ind.denominator, 1);
        assert_eq!(ind.numerator, 1);
        assert_eq!(ev.len(), 1);
        assert_eq!(ev[0].uti, "U1");
    }

    #[test]
    fn breach_above_red_threshold() {
        // amber=0.005, red=0.02 → 5/100 = 0.05 should be red
        let mut recs = Vec::new();
        for i in 0..100 {
            let v = if i < 5 {
                None
            } else {
                Some(Decimal::new(1, 0))
            };
            recs.push(rec(&format!("U{i}"), Some("OUTSTANDING"), v));
        }
        let (ind, ev) =
            compute_dqi_val_missing(&recs, &Thresholds::default(), MappingPresence::default());
        assert_eq!(ind.numerator, 5);
        assert_eq!(ind.denominator, 100);
        assert!((ind.rate.unwrap() - 0.05).abs() < 1e-9);
        assert_eq!(ind.status, DqiStatus::Red);
        assert_eq!(ev.len(), 5);
    }

    #[test]
    fn evidence_truncated_at_top_n() {
        let mut recs = Vec::new();
        for i in 0..50 {
            recs.push(rec(&format!("U{i:02}"), Some("OUTSTANDING"), None));
        }
        let (_ind, ev) =
            compute_dqi_val_missing(&recs, &Thresholds::default(), MappingPresence::default());
        assert_eq!(ev.len(), EVIDENCE_TOP_N);
        // Deterministically sorted on (source_file, uti); empty
        // source_file for all → ordered by UTI ascending.
        assert_eq!(ev.first().unwrap().uti, "U00");
        assert_eq!(ev.last().unwrap().uti, "U19");
    }
}
