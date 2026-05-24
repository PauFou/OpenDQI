//! `DQI_T3_EXCESS_COLLATERAL_USE_SFTR` — share of SFTR MSR
//! portfolios where the excess collateral reported (posted or
//! received) is **strictly greater than zero**.
//!
//! This is an SFTR-specific indicator (no EMIR equivalent —
//! EMIR's auth.109 MSR has no `XcssColl*` element). It measures
//! the operational pattern of over-collateralisation : a high
//! rate flags either a TR-side reporting inflation (excess
//! reported on every portfolio regardless of actual margining)
//! or a real operational waste (capital tied up in excess
//! collateral above the requirement).
//!
//! - **Layer:** SFTR MSR (`auth.085`).
//! - **Denominator:** records with at least one of the 6
//!   amounts set (IM/VM posted/received + the 2 excess
//!   amounts).
//! - **Numerator:** records where
//!   `excess_collateral_posted > 0` OR
//!   `excess_collateral_received > 0`.
//! - **Dimension:** accuracy.

use rust_decimal::Decimal;

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, Regime, SftrMarginStateRecord};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_T3_EXCESS_COLLATERAL_USE_SFTR";
const DESCRIPTION: &str = "Share of SFTR MSR portfolios reporting any excess collateral \
(excess_collateral_posted > 0 OR excess_collateral_received > 0). SFTR-specific accuracy indicator \
— flags TR-side reporting inflation or operational over-collateralisation.";

fn has_any_amount(r: &SftrMarginStateRecord) -> bool {
    r.initial_margin_posted.is_some()
        || r.variation_margin_posted.is_some()
        || r.excess_collateral_posted.is_some()
        || r.initial_margin_received.is_some()
        || r.variation_margin_received.is_some()
        || r.excess_collateral_received.is_some()
}

fn positive(d: Option<Decimal>) -> bool {
    d.map(|v| v > Decimal::ZERO).unwrap_or(false)
}

/// Compute `DQI_T3_EXCESS_COLLATERAL_USE_SFTR`.
pub fn compute_dqi_t3_excess_collateral_use_sftr(
    msr: &[SftrMarginStateRecord],
    thresholds: &Thresholds,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    let mut denominator: u64 = 0;
    let mut numerator: u64 = 0;
    let mut offenders: Vec<DqiEvidence> = Vec::new();

    for r in msr {
        if !has_any_amount(r) {
            continue;
        }
        denominator += 1;
        let pstd = positive(r.excess_collateral_posted);
        let rcvd = positive(r.excess_collateral_received);
        if !(pstd || rcvd) {
            continue;
        }
        numerator += 1;
        let portfolio = r
            .collateral_portfolio_code
            .clone()
            .unwrap_or_else(|| "(unknown portfolio)".into());
        let observed = match (r.excess_collateral_posted, r.excess_collateral_received) {
            (Some(p), Some(rc)) => format!("posted={p}, received={rc}"),
            (Some(p), None) => format!("posted={p}"),
            (None, Some(rc)) => format!("received={rc}"),
            (None, None) => String::new(),
        };
        offenders.push(DqiEvidence {
            indicator_id: INDICATOR_ID.into(),
            uti: portfolio,
            counterparty: r.reporting_counterparty.clone(),
            asset_class: None,
            source_file: r.source_file.clone(),
            observed_value: Some(observed),
            explanation: "excess collateral > 0 on posted or received side".into(),
        });
    }

    // Sort: largest absolute excess first so the worst offenders
    // make the top-20 evidence cut.
    offenders.sort_by(|a, b| b.observed_value.cmp(&a.observed_value));
    offenders.truncate(EVIDENCE_TOP_N);

    let pair = resolve_threshold(thresholds, INDICATOR_ID);
    let (rate, status) = rate_with_status(numerator, denominator, &pair);
    let indicator = DqiIndicator {
        indicator_id: INDICATOR_ID.into(),
        regime: Regime::Sftr,
        dimension: DqDimension::Accuracy,
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
    use super::*;
    use crate::dq::dqi::DqiStatus;

    fn rec(
        portfolio: &str,
        im_pst: Option<Decimal>,
        xcss_pst: Option<Decimal>,
        xcss_rcv: Option<Decimal>,
    ) -> SftrMarginStateRecord {
        SftrMarginStateRecord {
            collateral_portfolio_code: Some(portfolio.into()),
            initial_margin_posted: im_pst,
            excess_collateral_posted: xcss_pst,
            excess_collateral_received: xcss_rcv,
            ..Default::default()
        }
    }

    #[test]
    fn empty_is_not_applicable() {
        let (ind, _) = compute_dqi_t3_excess_collateral_use_sftr(
            &[],
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn records_with_no_amount_at_all_excluded() {
        // Pure metadata-only record (CtrPty + portfolio but no
        // margin block) doesn't count in the denominator.
        let r = SftrMarginStateRecord {
            collateral_portfolio_code: Some("P0".into()),
            ..Default::default()
        };
        let (ind, _) = compute_dqi_t3_excess_collateral_use_sftr(
            std::slice::from_ref(&r),
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 0);
    }

    #[test]
    fn no_excess_is_green() {
        let recs = vec![
            rec("P1", Some(Decimal::from(1000)), None, None),
            rec("P2", Some(Decimal::from(500)), Some(Decimal::ZERO), None),
        ];
        let (ind, _) = compute_dqi_t3_excess_collateral_use_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 2);
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn positive_excess_fires_on_either_side() {
        let recs = vec![
            rec(
                "P1",
                Some(Decimal::from(1000)),
                Some(Decimal::from(50)),
                None,
            ),
            rec(
                "P2",
                Some(Decimal::from(1000)),
                None,
                Some(Decimal::from(25)),
            ),
            rec("P3", Some(Decimal::from(1000)), None, None),
        ];
        let (ind, ev) = compute_dqi_t3_excess_collateral_use_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 3);
        assert_eq!(ind.numerator, 2);
        assert_eq!(ev.len(), 2);
    }

    #[test]
    fn excess_use_signal_from_real_a2_fixture_shape() {
        // Mirrors REC-4 of the v0.17 A2' fixture
        // (auth085-sample.xml): a portfolio with XcssCollPstd
        // = 500_000 vs IM = 100_000. The DQI fires.
        let r = SftrMarginStateRecord {
            collateral_portfolio_code: Some("PORTFOLIO-004".into()),
            initial_margin_posted: Some(Decimal::from(100_000)),
            excess_collateral_posted: Some(Decimal::from(500_000)),
            ..Default::default()
        };
        let (ind, _) = compute_dqi_t3_excess_collateral_use_sftr(
            std::slice::from_ref(&r),
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 1);
    }
}
