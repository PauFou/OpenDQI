//! `DQI_REUSE_STATE_STALE_SFTR` — share of SFTR auth.086 state
//! records whose `state_as_of` is older than the configured
//! TARGET2 business-day threshold vs the run's `as_of`.
//!
//! Mirror of `DQI_T3_MARGIN_STALE_SFTR` (auth.085 MSR state),
//! adapted to the reuse-state schema. Reuses the same
//! [`crate::config::TimelinessThresholds::max_valuation_age_business_days`]
//! field — staleness intuition transfers across SFTR state
//! messages.
//!
//! - **Layer:** SFTR Reuse State (`auth.086`).
//! - **Denominator:** records with `state_as_of` set AND at
//!   least one reuse content field populated (Scty/ReuseVal
//!   summed into `total_reuse_value`, or
//!   `cash_reinvestment_rate`). Records with no content are
//!   excluded — staleness on an empty snapshot is uninformative.
//! - **Numerator:** records whose `state_as_of` is strictly
//!   older than `max_valuation_age_business_days` TARGET2
//!   business days from `as_of`.
//! - **Dimension:** timeliness.

use chrono::NaiveDate;

use crate::business_days::business_day_diff;
use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, Regime, SftrReuseStateRecord};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_REUSE_STATE_STALE_SFTR";
const DESCRIPTION: &str = "Share of SFTR auth.086 state snapshots whose state_as_of is older \
than the configured TARGET2 business-day threshold (max_valuation_age_business_days, reused \
from the EMIR TimelinessThresholds). Counts only records carrying at least one reuse content \
field.";

fn has_reuse_content(r: &SftrReuseStateRecord) -> bool {
    r.total_reuse_value.is_some() || r.cash_reinvestment_rate.is_some()
}

/// Compute `DQI_REUSE_STATE_STALE_SFTR`.
pub fn compute_dqi_reuse_state_stale_sftr(
    reuse_state: &[SftrReuseStateRecord],
    thresholds: &Thresholds,
    as_of: NaiveDate,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    let max_age = thresholds.timeliness.max_valuation_age_business_days.max(0);

    let mut denominator: u64 = 0;
    let mut numerator: u64 = 0;
    let mut offenders: Vec<DqiEvidence> = Vec::new();

    for r in reuse_state {
        let Some(ts) = r.state_as_of else { continue };
        if !has_reuse_content(r) {
            continue;
        }
        denominator += 1;
        let age = business_day_diff(ts.date_naive(), as_of);
        if age <= max_age {
            continue;
        }
        numerator += 1;
        let label = r.record_id.clone().unwrap_or_else(|| "(unknown)".into());
        offenders.push(DqiEvidence {
            indicator_id: INDICATOR_ID.into(),
            uti: label,
            counterparty: r.reporting_counterparty.clone(),
            asset_class: None,
            source_file: r.source_file.clone(),
            observed_value: Some(ts.to_rfc3339()),
            explanation: format!(
                "state_as_of older than {max_age} TARGET2 business day(s) vs as_of {as_of}"
            ),
        });
    }

    // Oldest first.
    offenders.sort_by(|a, b| a.observed_value.cmp(&b.observed_value));
    offenders.truncate(EVIDENCE_TOP_N);

    let pair = resolve_threshold(thresholds, INDICATOR_ID);
    let (rate, status) = rate_with_status(numerator, denominator, &pair);
    let indicator = DqiIndicator {
        indicator_id: INDICATOR_ID.into(),
        regime: Regime::Sftr,
        dimension: DqDimension::Timeliness,
        table_scope: "SFTR-REU-STATE".into(),
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
    use rust_decimal::Decimal;

    use super::*;
    use crate::dq::dqi::DqiStatus;

    fn rec(
        id: &str,
        state_as_of: Option<DateTime<Utc>>,
        with_content: bool,
    ) -> SftrReuseStateRecord {
        SftrReuseStateRecord {
            record_id: Some(id.into()),
            state_as_of,
            total_reuse_value: with_content.then(|| Decimal::from(1000)),
            ..Default::default()
        }
    }

    fn d(y: i32, m: u32, day: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, day, 0, 0, 0).single().unwrap()
    }

    fn as_of() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 5, 21).unwrap()
    }

    #[test]
    fn empty_is_not_applicable() {
        let (ind, _) = compute_dqi_reuse_state_stale_sftr(
            &[],
            &Thresholds::default(),
            as_of(),
            MappingPresence::default(),
        );
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn records_without_state_as_of_excluded() {
        let r = SftrReuseStateRecord {
            record_id: Some("R-NO-TS".into()),
            total_reuse_value: Some(Decimal::from(100)),
            ..Default::default()
        };
        let (ind, _) = compute_dqi_reuse_state_stale_sftr(
            std::slice::from_ref(&r),
            &Thresholds::default(),
            as_of(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 0);
    }

    #[test]
    fn records_without_reuse_content_excluded() {
        // state_as_of set but no Scty / Csh — denominator
        // should skip (staleness on an empty snapshot is
        // uninformative).
        let r = SftrReuseStateRecord {
            record_id: Some("R-EMPTY".into()),
            state_as_of: Some(d(2020, 1, 1)),
            ..Default::default()
        };
        let (ind, _) = compute_dqi_reuse_state_stale_sftr(
            std::slice::from_ref(&r),
            &Thresholds::default(),
            as_of(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 0);
    }

    #[test]
    fn fresh_snapshot_is_green() {
        // state_as_of = today → age 0 BD → below threshold → green.
        let recs = vec![rec("R1", Some(d(2026, 5, 21)), true)];
        let (ind, _) = compute_dqi_reuse_state_stale_sftr(
            &recs,
            &Thresholds::default(),
            as_of(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 1);
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn very_old_snapshot_is_stale_with_evidence() {
        // state_as_of = 2020-01-01 → vastly older than the
        // default threshold → fires.
        let recs = vec![rec("R-STALE", Some(d(2020, 1, 1)), true)];
        let (ind, ev) = compute_dqi_reuse_state_stale_sftr(
            &recs,
            &Thresholds::default(),
            as_of(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 1);
        assert_eq!(ev.len(), 1);
        assert_eq!(ev[0].uti, "R-STALE");
    }
}
