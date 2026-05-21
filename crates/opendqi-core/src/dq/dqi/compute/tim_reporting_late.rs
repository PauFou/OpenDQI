//! `DQI_TIM_REPORTING_LATE` — share of TAR records (modelled as
//! [`crate::model::EmirRecord`]) where the gap between
//! `execution_timestamp` and `reporting_timestamp` exceeds the
//! configured `max_reporting_delay_hours`.
//!
//! Mirrors the per-row `EMIR.TIM.LATE_REPORTING` check at the
//! indicator level — same threshold semantics.
//!
//! - **Layer:** TAR (modelled as `EmirRecord`).
//! - **Denominator:** records with **both** timestamps set.
//! - **Numerator:** records where
//!   `reporting_timestamp - execution_timestamp > threshold`.
//! - **Dimension:** timeliness.

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, EmirRecord, Regime};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_TIM_REPORTING_LATE";
const DESCRIPTION: &str = "TAR records whose reporting_timestamp lags execution_timestamp by \
more than the configured max_reporting_delay_hours.";

/// Compute `DQI_TIM_REPORTING_LATE` on a TAR snapshot.
pub fn compute_dqi_tim_reporting_late(
    tar: &[EmirRecord],
    thresholds: &Thresholds,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    let max_hours = thresholds.timeliness.max_reporting_delay_hours.max(0);

    let mut denominator: u64 = 0;
    let mut numerator: u64 = 0;
    let mut offenders: Vec<DqiEvidence> = Vec::new();

    for r in tar {
        let (Some(exec), Some(rep)) = (r.execution_timestamp, r.reporting_timestamp) else {
            continue;
        };
        denominator += 1;
        let delta_hours = (rep - exec).num_hours();
        if delta_hours <= max_hours {
            continue;
        }
        numerator += 1;
        offenders.push(DqiEvidence {
            indicator_id: INDICATOR_ID.into(),
            uti: r.uti.clone().unwrap_or_default(),
            counterparty: r.entity_responsible_for_reporting.clone(),
            asset_class: r.asset_class.clone(),
            source_file: r.source_file.clone(),
            observed_value: Some(format!("{delta_hours}h")),
            explanation: format!(
                "reporting_timestamp − execution_timestamp = {delta_hours} hours \
(threshold {max_hours} h)"
            ),
        });
    }

    // Biggest delay first — most useful for triage.
    offenders.sort_by(|a, b| {
        // observed_value is "Nh" — strip and compare numerically.
        fn hours_of(s: &Option<String>) -> i64 {
            s.as_deref()
                .and_then(|v| v.trim_end_matches('h').parse::<i64>().ok())
                .unwrap_or(0)
        }
        hours_of(&b.observed_value).cmp(&hours_of(&a.observed_value))
    });
    offenders.truncate(EVIDENCE_TOP_N);

    let pair = resolve_threshold(thresholds, INDICATOR_ID);
    let (rate, status) = rate_with_status(numerator, denominator, &pair);
    let indicator = DqiIndicator {
        indicator_id: INDICATOR_ID.into(),
        regime: Regime::Emir,
        dimension: DqDimension::Timeliness,
        table_scope: "TAR".into(),
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
    use chrono::{DateTime, TimeZone, Utc};

    use super::*;
    use crate::dq::dqi::DqiStatus;

    fn ts(y: i32, m: u32, d: u32, h: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, h, 0, 0).single().unwrap()
    }

    fn rec(uti: &str, exec: Option<DateTime<Utc>>, rep: Option<DateTime<Utc>>) -> EmirRecord {
        EmirRecord {
            uti: Some(uti.into()),
            execution_timestamp: exec,
            reporting_timestamp: rep,
            ..Default::default()
        }
    }

    #[test]
    fn empty_input_is_not_applicable() {
        let (ind, _) =
            compute_dqi_tim_reporting_late(&[], &Thresholds::default(), MappingPresence::default());
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn missing_either_timestamp_excludes_record() {
        let recs = vec![
            rec("U1", Some(ts(2026, 5, 20, 9)), None),
            rec("U2", None, Some(ts(2026, 5, 20, 9))),
        ];
        let (ind, _) = compute_dqi_tim_reporting_late(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 0);
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn within_threshold_is_green() {
        // default max_reporting_delay_hours = 24
        let recs = vec![
            rec("U1", Some(ts(2026, 5, 20, 9)), Some(ts(2026, 5, 20, 15))), // 6h
            rec("U2", Some(ts(2026, 5, 20, 0)), Some(ts(2026, 5, 21, 0))),  // 24h exactly
        ];
        let (ind, _) = compute_dqi_tim_reporting_late(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.denominator, 2);
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn beyond_threshold_breaches_and_sorts_by_delay() {
        let recs = vec![
            rec("U1", Some(ts(2026, 5, 20, 0)), Some(ts(2026, 5, 21, 6))), // 30h
            rec("U2", Some(ts(2026, 5, 18, 0)), Some(ts(2026, 5, 21, 0))), // 72h
            rec("U3", Some(ts(2026, 5, 20, 0)), Some(ts(2026, 5, 20, 1))), // 1h fresh
        ];
        let (ind, ev) = compute_dqi_tim_reporting_late(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 3);
        assert_eq!(ind.numerator, 2);
        // Biggest delay first
        assert_eq!(ev[0].uti, "U2");
        assert_eq!(ev[1].uti, "U1");
    }
}
