//! `DQI_VAL_STALE` — share of TSR records whose valuation
//! timestamp is older than [`crate::config::TimelinessThresholds::max_valuation_age_business_days`]
//! relative to `as_of`.
//!
//! - **Layer:** TSR.
//! - **Denominator:** TSR records with a `valuation_timestamp`
//!   set (records without a timestamp are excluded — counted by
//!   `DQI_VAL_MISSING` instead).
//! - **Numerator:** records whose timestamp is older than the
//!   threshold age **in calendar days** (proxy for business
//!   days in v0.15; business-day calendar awareness = v0.16).
//! - **Dimension:** timeliness.

use chrono::{DateTime, Duration, NaiveDate, Utc};

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, Regime, TrStateRecord};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_VAL_STALE";
const DESCRIPTION: &str = "TSR records whose valuation timestamp is older than the configured \
threshold (calendar-day proxy in v0.15).";

/// Compute `DQI_VAL_STALE` on a TSR snapshot.
///
/// `as_of` is the reference date; defaults to `Utc::now`-date
/// at the orchestrator level.
pub fn compute_dqi_val_stale(
    tsr: &[TrStateRecord],
    thresholds: &Thresholds,
    as_of: NaiveDate,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    let max_age_days = thresholds.timeliness.max_valuation_age_business_days.max(0);
    // Convert as_of to midnight UTC → "anything before this minus
    // max_age days" is stale.
    let cutoff: DateTime<Utc> = as_of
        .and_hms_opt(0, 0, 0)
        .unwrap_or_else(|| {
            NaiveDate::from_ymd_opt(1970, 1, 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap()
        })
        .and_utc()
        - Duration::days(max_age_days);

    let mut denominator: u64 = 0;
    let mut numerator: u64 = 0;
    let mut offenders: Vec<DqiEvidence> = Vec::new();

    for r in tsr {
        let Some(ts) = r.valuation_timestamp else {
            continue;
        };
        denominator += 1;
        if ts >= cutoff {
            continue;
        }
        numerator += 1;
        offenders.push(DqiEvidence {
            indicator_id: INDICATOR_ID.into(),
            uti: r.uti.clone().unwrap_or_default(),
            counterparty: r.reporting_counterparty.clone(),
            asset_class: None,
            source_file: r.source_file.clone(),
            observed_value: Some(ts.to_rfc3339()),
            explanation: format!(
                "valuation_timestamp older than {} day(s) vs as_of {}",
                max_age_days, as_of
            ),
        });
    }

    // Oldest first (worst offenders at top).
    offenders.sort_by(|a, b| a.observed_value.cmp(&b.observed_value));
    offenders.truncate(EVIDENCE_TOP_N);

    let pair = resolve_threshold(thresholds, INDICATOR_ID);
    let (rate, status) = rate_with_status(numerator, denominator, &pair);
    let indicator = DqiIndicator {
        indicator_id: INDICATOR_ID.into(),
        regime: Regime::Emir,
        dimension: DqDimension::Timeliness,
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
    use chrono::TimeZone;

    use super::*;
    use crate::dq::dqi::DqiStatus;

    fn ts(y: i32, m: u32, d: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, 0, 0, 0).single().unwrap()
    }

    fn rec(uti: &str, valuation_ts: Option<DateTime<Utc>>) -> TrStateRecord {
        TrStateRecord {
            uti: Some(uti.into()),
            valuation_timestamp: valuation_ts,
            ..Default::default()
        }
    }

    fn as_of() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 5, 21).unwrap()
    }

    #[test]
    fn empty_input_is_not_applicable() {
        let (ind, ev) = compute_dqi_val_stale(
            &[],
            &Thresholds::default(),
            as_of(),
            MappingPresence::default(),
        );
        assert_eq!(ind.status, DqiStatus::NotApplicable);
        assert_eq!(ind.denominator, 0);
        assert!(ev.is_empty());
    }

    #[test]
    fn missing_timestamp_excluded_from_denominator() {
        let recs = vec![rec("U1", None), rec("U2", None)];
        let (ind, _) = compute_dqi_val_stale(
            &recs,
            &Thresholds::default(),
            as_of(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 0);
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn fresh_timestamps_are_green() {
        // Default max_valuation_age_business_days = 1 ; as_of =
        // 2026-05-21 → cutoff = 2026-05-20 00:00 UTC. Timestamps
        // ≥ cutoff are fresh.
        let recs = vec![
            rec("U1", Some(ts(2026, 5, 20))),
            rec("U2", Some(ts(2026, 5, 21))),
        ];
        let (ind, _) = compute_dqi_val_stale(
            &recs,
            &Thresholds::default(),
            as_of(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.denominator, 2);
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn old_timestamps_are_stale() {
        let recs = vec![
            rec("U1", Some(ts(2026, 5, 19))), // 2 days old > threshold of 1 day
            rec("U2", Some(ts(2026, 1, 1))),
            rec("U3", Some(ts(2026, 5, 21))), // fresh
        ];
        let (ind, ev) = compute_dqi_val_stale(
            &recs,
            &Thresholds::default(),
            as_of(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 3);
        assert_eq!(ind.numerator, 2);
        assert!((ind.rate.unwrap() - 2.0 / 3.0).abs() < 1e-9);
        assert_eq!(ind.status, DqiStatus::Red);
        // Oldest first
        assert_eq!(ev[0].uti, "U2");
        assert_eq!(ev[1].uti, "U1");
    }
}
