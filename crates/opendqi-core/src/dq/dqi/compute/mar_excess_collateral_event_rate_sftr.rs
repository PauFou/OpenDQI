//! `DQI_MAR_EXCESS_COLLATERAL_EVENT_RATE_SFTR` — share of SFTR
//! MAR events (auth.070 records) that report **excess collateral
//! posted > 0**.
//!
//! - **Layer:** SFTR MAR (`auth.070`).
//! - **Denominator:** records that carry any posted-side amount
//!   (`initial_margin_posted`, `variation_margin_posted`, or
//!   `excess_collateral_posted` is `Some`). The `Err` wrapper
//!   (action_type=`ERRT`) is naturally excluded (no amounts).
//! - **Numerator:** records where `excess_collateral_posted >
//!   0`. A `Some(Decimal::ZERO)` does **not** count — only
//!   strictly positive excess collateral signals operational
//!   activity worth flagging.
//! - **Dimension:** accuracy (operational signal).
//!
//! Note: this is the activity-side mirror of
//! `DQI_T3_EXCESS_COLLATERAL_USE_SFTR` (auth.085) — the state
//! snapshot measures the share of portfolios *currently*
//! reporting excess collateral, while this measures the share
//! of *individual events* posting excess. Together they
//! triangulate the operational dimension. Threshold pair
//! mirrors the state-side default (20 % amber / 50 % red).

use rust_decimal::Decimal;

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, Regime, SftrMarginActivityRecord};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_MAR_EXCESS_COLLATERAL_EVENT_RATE_SFTR";
const DESCRIPTION: &str =
    "Share of SFTR MAR events (auth.070) that report excess_collateral_posted > 0. Activity-side \
mirror of DQI_T3_EXCESS_COLLATERAL_USE_SFTR (auth.085). High rates suggest over-collateralisation \
churn or systematic margin-call mis-sizing on the posted side.";

#[inline]
fn has_any_posted(r: &SftrMarginActivityRecord) -> bool {
    r.initial_margin_posted.is_some()
        || r.variation_margin_posted.is_some()
        || r.excess_collateral_posted.is_some()
}

/// Compute `DQI_MAR_EXCESS_COLLATERAL_EVENT_RATE_SFTR`.
pub fn compute_dqi_mar_excess_collateral_event_rate_sftr(
    mar: &[SftrMarginActivityRecord],
    thresholds: &Thresholds,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    let mut denominator: u64 = 0;
    let mut numerator: u64 = 0;
    let mut offenders: Vec<DqiEvidence> = Vec::new();

    for r in mar {
        if !has_any_posted(r) {
            continue;
        }
        denominator += 1;
        match r.excess_collateral_posted {
            Some(v) if v > Decimal::ZERO => {
                numerator += 1;
                let portfolio = r
                    .collateral_portfolio_code
                    .as_deref()
                    .unwrap_or("<no-portfolio>");
                offenders.push(DqiEvidence {
                    indicator_id: INDICATOR_ID.into(),
                    uti: portfolio.to_owned(),
                    counterparty: r.reporting_counterparty.clone(),
                    asset_class: None,
                    source_file: r.source_file.clone(),
                    observed_value: Some(v.to_string()),
                    explanation: format!(
                        "MAR event posts excess collateral {v} {ccy}",
                        ccy = r.margin_currency.as_deref().unwrap_or("(no Ccy)")
                    ),
                });
            }
            _ => {}
        }
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
        regime: Regime::Sftr,
        dimension: DqDimension::Accuracy,
        table_scope: "SFTR-MAR".into(),
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

    fn rec(
        portfolio: &str,
        im_pst: Option<Decimal>,
        xcss_pst: Option<Decimal>,
    ) -> SftrMarginActivityRecord {
        SftrMarginActivityRecord {
            collateral_portfolio_code: Some(portfolio.into()),
            initial_margin_posted: im_pst,
            excess_collateral_posted: xcss_pst,
            ..Default::default()
        }
    }

    #[test]
    fn empty_is_not_applicable() {
        let (ind, _) = compute_dqi_mar_excess_collateral_event_rate_sftr(
            &[],
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn no_posted_records_yield_empty_denominator() {
        // Received-only records have no posted side → excluded.
        let r = SftrMarginActivityRecord {
            collateral_portfolio_code: Some("P-RCV".into()),
            initial_margin_received: Some(Decimal::from(100)),
            ..Default::default()
        };
        let (ind, _) = compute_dqi_mar_excess_collateral_event_rate_sftr(
            std::slice::from_ref(&r),
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 0);
    }

    #[test]
    fn zero_excess_does_not_fire() {
        let recs = vec![
            rec("P1", Some(Decimal::from(100)), None),
            rec("P2", Some(Decimal::from(200)), Some(Decimal::ZERO)),
        ];
        let (ind, _) = compute_dqi_mar_excess_collateral_event_rate_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 2);
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn positive_excess_fires_with_evidence() {
        let recs = vec![
            rec("P1", Some(Decimal::from(100)), Some(Decimal::from(50))),
            rec("P2", Some(Decimal::from(200)), None),
            rec("P3", Some(Decimal::from(100)), Some(Decimal::from(75))),
        ];
        let (ind, ev) = compute_dqi_mar_excess_collateral_event_rate_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 3);
        assert_eq!(ind.numerator, 2);
        assert_eq!(ev.len(), 2);
        assert_eq!(ev[0].uti, "P1");
        assert_eq!(ev[1].uti, "P3");
    }

    #[test]
    fn rate_above_red_threshold_is_red() {
        // 3 / 3 = 100 % posts excess → well above the 50 % red.
        let recs = vec![
            rec("P1", Some(Decimal::from(100)), Some(Decimal::from(1))),
            rec("P2", Some(Decimal::from(100)), Some(Decimal::from(1))),
            rec("P3", Some(Decimal::from(100)), Some(Decimal::from(1))),
        ];
        let (ind, _) = compute_dqi_mar_excess_collateral_event_rate_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 3);
        assert_eq!(ind.denominator, 3);
        assert_eq!(ind.status, DqiStatus::Red);
    }
}
