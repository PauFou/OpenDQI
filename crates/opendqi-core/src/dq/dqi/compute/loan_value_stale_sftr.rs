//! `DQI_LOAN_VALUE_STALE_SFTR` — share of SFTR TSR records
//! whose snapshot `state_as_of` is older than the configured
//! threshold (TARGET2 business days).
//!
//! SFTR mirror of EMIR's [`crate::dq::dqi::compute_dqi_val_stale`].
//! Uses `state_as_of` as the freshness anchor because SFTR
//! does not carry a separate `valuation_timestamp` per record
//! — the per-state snapshot timestamp at the report level is
//! the only TR-side freshness signal.
//!
//! - **Layer:** SFTR TSR.
//! - **Denominator:** records with `state_as_of` set.
//! - **Numerator:** records whose `state_as_of` is older than
//!   [`crate::config::TimelinessThresholds::max_valuation_age_business_days`]
//!   (reused EMIR threshold — SFTR uses the same number of
//!   business days for v0.16).
//! - **Dimension:** timeliness.

use chrono::NaiveDate;

use crate::business_days::business_day_diff;
use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, Regime, SftrTrStateRecord};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_LOAN_VALUE_STALE_SFTR";
const DESCRIPTION: &str = "SFTR TSR records whose state_as_of is older than the configured \
threshold (TARGET2 business days). Reuses the EMIR \
TimelinessThresholds.max_valuation_age_business_days config.";

/// Compute `DQI_LOAN_VALUE_STALE_SFTR`.
pub fn compute_dqi_loan_value_stale_sftr(
    tsr: &[SftrTrStateRecord],
    thresholds: &Thresholds,
    as_of: NaiveDate,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    let max_age = thresholds.timeliness.max_valuation_age_business_days.max(0);

    let mut denominator: u64 = 0;
    let mut numerator: u64 = 0;
    let mut offenders: Vec<DqiEvidence> = Vec::new();

    for r in tsr {
        let Some(ts) = r.state_as_of else { continue };
        denominator += 1;
        let age = business_day_diff(ts.date_naive(), as_of);
        if age <= max_age {
            continue;
        }
        numerator += 1;
        offenders.push(DqiEvidence {
            indicator_id: INDICATOR_ID.into(),
            uti: r.uti.clone().unwrap_or_default(),
            counterparty: r.reporting_counterparty.clone(),
            asset_class: r.sft_type.clone(),
            source_file: r.source_file.clone(),
            observed_value: Some(ts.to_rfc3339()),
            explanation: format!(
                "state_as_of older than {max_age} TARGET2 business day(s) vs as_of {as_of}"
            ),
        });
    }

    offenders.sort_by(|a, b| a.observed_value.cmp(&b.observed_value));
    offenders.truncate(EVIDENCE_TOP_N);

    let pair = resolve_threshold(thresholds, INDICATOR_ID);
    let (rate, status) = rate_with_status(numerator, denominator, &pair);
    let indicator = DqiIndicator {
        indicator_id: INDICATOR_ID.into(),
        regime: Regime::Sftr,
        dimension: DqDimension::Timeliness,
        table_scope: "SFTR-TSR".into(),
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

    fn rec(uti: &str, state_as_of: Option<DateTime<Utc>>) -> SftrTrStateRecord {
        SftrTrStateRecord {
            uti: Some(uti.into()),
            state_as_of,
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
        let (ind, _) = compute_dqi_loan_value_stale_sftr(
            &[],
            &Thresholds::default(),
            as_of(),
            MappingPresence::default(),
        );
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn fresh_is_green() {
        let recs = vec![rec("U1", Some(d(2026, 5, 21)))];
        let (ind, _) = compute_dqi_loan_value_stale_sftr(
            &recs,
            &Thresholds::default(),
            as_of(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn stale_fires() {
        let recs = vec![
            rec("U1", Some(d(2026, 1, 1))),  // very old
            rec("U2", Some(d(2026, 5, 21))), // fresh
        ];
        let (ind, _) = compute_dqi_loan_value_stale_sftr(
            &recs,
            &Thresholds::default(),
            as_of(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 1);
        assert_eq!(ind.status, DqiStatus::Red);
    }
}
