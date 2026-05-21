//! `DQI_COL_STALE_STATE` — share of MSR records whose
//! `state_as_of` is older than the configured threshold.
//!
//! Mirrors the `EMIR.COL.STALE` check (see
//! `dq/collateral_audit.rs`) at the indicator-level: instead of
//! flagging individual records, this aggregates the share of
//! stale collateral states across the whole MSR.
//!
//! - **Layer:** MSR.
//! - **Denominator:** MSR records with `state_as_of` set.
//! - **Numerator:** records older than
//!   [`crate::config::EmirRmtThresholds::collateral_max_age_days`]
//!   relative to `as_of`.
//! - **Dimension:** timeliness.

use chrono::{DateTime, Duration, NaiveDate, Utc};

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, MarginStateRecord, Regime};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_COL_STALE_STATE";
const DESCRIPTION: &str = "MSR records whose state_as_of is older than the configured \
collateral_max_age_days vs the as_of reference date.";

/// Compute `DQI_COL_STALE_STATE` on an MSR snapshot.
pub fn compute_dqi_col_stale_state(
    msr: &[MarginStateRecord],
    thresholds: &Thresholds,
    as_of: NaiveDate,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    let max_age_days = thresholds.emir_rmt.collateral_max_age_days.max(0);
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

    for r in msr {
        let Some(ts) = r.state_as_of else {
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
            counterparty: r.counterparty_1.clone(),
            asset_class: None,
            source_file: r.source_file.clone(),
            observed_value: Some(ts.to_rfc3339()),
            explanation: format!(
                "state_as_of older than {} day(s) vs as_of {}",
                max_age_days, as_of
            ),
        });
    }

    offenders.sort_by(|a, b| a.observed_value.cmp(&b.observed_value));
    offenders.truncate(EVIDENCE_TOP_N);

    let pair = resolve_threshold(thresholds, INDICATOR_ID);
    let (rate, status) = rate_with_status(numerator, denominator, &pair);
    let indicator = DqiIndicator {
        indicator_id: INDICATOR_ID.into(),
        regime: Regime::Emir,
        dimension: DqDimension::Timeliness,
        table_scope: "MSR".into(),
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

    fn rec(uti: &str, state_as_of: Option<DateTime<Utc>>) -> MarginStateRecord {
        MarginStateRecord {
            uti: Some(uti.into()),
            state_as_of,
            ..Default::default()
        }
    }

    fn as_of() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 5, 21).unwrap()
    }

    #[test]
    fn empty_input_is_not_applicable() {
        let (ind, _) = compute_dqi_col_stale_state(
            &[],
            &Thresholds::default(),
            as_of(),
            MappingPresence::default(),
        );
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn fresh_state_is_green() {
        let recs = vec![
            rec("U1", Some(ts(2026, 5, 21))),
            rec("U2", Some(ts(2026, 5, 20))),
        ];
        let (ind, _) = compute_dqi_col_stale_state(
            &recs,
            &Thresholds::default(),
            as_of(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn old_state_is_breach() {
        let recs = vec![
            rec("U1", Some(ts(2026, 4, 1))),
            rec("U2", Some(ts(2026, 5, 21))),
        ];
        let (ind, ev) = compute_dqi_col_stale_state(
            &recs,
            &Thresholds::default(),
            as_of(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 1);
        assert_eq!(ind.denominator, 2);
        assert_eq!(ev[0].uti, "U1");
    }
}
