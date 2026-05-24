//! `DQI_T3_MARGIN_STALE_SFTR` — share of SFTR MSR portfolios
//! with stale margin state (the snapshot `state_as_of` is older
//! than the configured business-day threshold and the record
//! carries at least one margin amount).
//!
//! SFTR mirror of EMIR's [`crate::dq::dqi::compute_dqi_col_stale_state`]
//! for the auth.085 portfolio-level layer. Uses the same
//! [`crate::config::TimelinessThresholds::max_valuation_age_business_days`]
//! config key as the existing SFTR TSR stale-loan-value DQI
//! (`DQI_LOAN_VALUE_STALE_SFTR`).
//!
//! - **Layer:** SFTR MSR (`auth.085`).
//! - **Denominator:** records with `state_as_of` set AND at
//!   least one of the 6 margin amounts set.
//! - **Numerator:** records whose `state_as_of` is older than
//!   the configured TARGET2 business-day threshold vs `as_of`.
//! - **Dimension:** timeliness.

use chrono::NaiveDate;

use crate::business_days::business_day_diff;
use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, Regime, SftrMarginStateRecord};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_T3_MARGIN_STALE_SFTR";
const DESCRIPTION: &str = "Share of SFTR MSR portfolios whose state_as_of is older than the \
configured TARGET2 business-day threshold (max_valuation_age_business_days, reused from the \
EMIR TimelinessThresholds). Counts only records carrying at least one margin amount.";

fn has_any_amount(r: &SftrMarginStateRecord) -> bool {
    r.initial_margin_posted.is_some()
        || r.variation_margin_posted.is_some()
        || r.excess_collateral_posted.is_some()
        || r.initial_margin_received.is_some()
        || r.variation_margin_received.is_some()
        || r.excess_collateral_received.is_some()
}

/// Compute `DQI_T3_MARGIN_STALE_SFTR`.
pub fn compute_dqi_t3_margin_stale_sftr(
    msr: &[SftrMarginStateRecord],
    thresholds: &Thresholds,
    as_of: NaiveDate,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    let max_age = thresholds.timeliness.max_valuation_age_business_days.max(0);

    let mut denominator: u64 = 0;
    let mut numerator: u64 = 0;
    let mut offenders: Vec<DqiEvidence> = Vec::new();

    for r in msr {
        let Some(ts) = r.state_as_of else { continue };
        if !has_any_amount(r) {
            continue;
        }
        denominator += 1;
        let age = business_day_diff(ts.date_naive(), as_of);
        if age <= max_age {
            continue;
        }
        numerator += 1;
        let portfolio = r
            .collateral_portfolio_code
            .clone()
            .unwrap_or_else(|| "(unknown portfolio)".into());
        offenders.push(DqiEvidence {
            indicator_id: INDICATOR_ID.into(),
            uti: portfolio,
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
        table_scope: "SFTR-MSR".into(),
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
        portfolio: &str,
        state_as_of: Option<DateTime<Utc>>,
        with_amount: bool,
    ) -> SftrMarginStateRecord {
        SftrMarginStateRecord {
            collateral_portfolio_code: Some(portfolio.into()),
            state_as_of,
            initial_margin_posted: with_amount.then(|| Decimal::from(100)),
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
        let (ind, _) = compute_dqi_t3_margin_stale_sftr(
            &[],
            &Thresholds::default(),
            as_of(),
            MappingPresence::default(),
        );
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn records_without_state_as_of_excluded() {
        let r = rec("P1", None, true);
        let (ind, _) = compute_dqi_t3_margin_stale_sftr(
            std::slice::from_ref(&r),
            &Thresholds::default(),
            as_of(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 0);
    }

    #[test]
    fn records_without_any_margin_amount_excluded() {
        // A portfolio with state_as_of but no amount whatsoever
        // is out of scope — there's no margin to call stale.
        // The completeness gap is captured by
        // DQI_T3_MARGIN_POSTED_MISSING / RECEIVED_MISSING.
        let r = rec("P1", Some(d(2026, 1, 1)), false);
        let (ind, _) = compute_dqi_t3_margin_stale_sftr(
            std::slice::from_ref(&r),
            &Thresholds::default(),
            as_of(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 0);
    }

    #[test]
    fn fresh_is_green() {
        let recs = vec![rec("P1", Some(d(2026, 5, 21)), true)];
        let (ind, _) = compute_dqi_t3_margin_stale_sftr(
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
            rec("P1", Some(d(2026, 1, 1)), true),  // very old
            rec("P2", Some(d(2026, 5, 21)), true), // fresh
        ];
        let (ind, _) = compute_dqi_t3_margin_stale_sftr(
            &recs,
            &Thresholds::default(),
            as_of(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 2);
        assert_eq!(ind.numerator, 1);
    }
}
