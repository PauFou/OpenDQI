//! `DQI_TIM_REPORTING_LATE_SFTR` — share of SFTR TAR records
//! ([`crate::SftrRecord`]) where the reporting timestamp lags
//! the execution timestamp beyond the configured threshold.
//!
//! SFTR mirror of EMIR's
//! [`crate::dq::dqi::compute_dqi_tim_reporting_late`]. Same
//! `Thresholds.timeliness.max_reporting_delay_hours` config.
//!
//! - **Layer:** SFTR TAR (`auth.052`).
//! - **Denominator:** records with both `execution_timestamp`
//!   and `reporting_timestamp` set.
//! - **Numerator:** records where the gap exceeds
//!   `max_reporting_delay_hours` (default 24).
//! - **Dimension:** timeliness.

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, Regime, SftrRecord};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_TIM_REPORTING_LATE_SFTR";
const DESCRIPTION: &str = "SFTR TAR records whose reporting_timestamp lags execution_timestamp \
by more than the configured max_reporting_delay_hours.";

/// Compute `DQI_TIM_REPORTING_LATE_SFTR`.
pub fn compute_dqi_tim_reporting_late_sftr(
    tar: &[SftrRecord],
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
            asset_class: r.sft_type.clone(),
            source_file: r.source_file.clone(),
            observed_value: Some(format!("{delta_hours}h")),
            explanation: format!(
                "reporting_timestamp − execution_timestamp = {delta_hours} hours (threshold {max_hours} h)"
            ),
        });
    }

    // Biggest delay first.
    offenders.sort_by(|a, b| {
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
        regime: Regime::Sftr,
        dimension: DqDimension::Timeliness,
        table_scope: "SFTR-TAR".into(),
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

    fn rec(uti: &str, exec: Option<DateTime<Utc>>, rep: Option<DateTime<Utc>>) -> SftrRecord {
        SftrRecord {
            uti: Some(uti.into()),
            execution_timestamp: exec,
            reporting_timestamp: rep,
            ..Default::default()
        }
    }

    #[test]
    fn empty_is_not_applicable() {
        let (ind, _) = compute_dqi_tim_reporting_late_sftr(
            &[],
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn within_threshold_is_green() {
        let recs = vec![rec(
            "U1",
            Some(ts(2026, 5, 20, 9)),
            Some(ts(2026, 5, 20, 15)),
        )];
        let (ind, _) = compute_dqi_tim_reporting_late_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn beyond_threshold_fires() {
        let recs = vec![
            rec("U1", Some(ts(2026, 5, 18, 0)), Some(ts(2026, 5, 21, 0))), // 72h
            rec("U2", Some(ts(2026, 5, 20, 0)), Some(ts(2026, 5, 20, 6))), // 6h fresh
        ];
        let (ind, ev) = compute_dqi_tim_reporting_late_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 2);
        assert_eq!(ind.numerator, 1);
        assert_eq!(ev[0].uti, "U1");
    }
}
