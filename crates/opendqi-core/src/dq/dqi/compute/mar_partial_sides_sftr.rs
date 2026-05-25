//! `DQI_MAR_PARTIAL_SIDES_SFTR` — share of SFTR MAR events
//! (auth.070 records) that report **only one of the two
//! posted / received sides**.
//!
//! - **Layer:** SFTR MAR (`auth.070`).
//! - **Denominator:** records that carry at least one amount on
//!   either side. The `Err` wrapper (action_type=`ERRT`) is
//!   metadata-only at the XSD level and is naturally excluded
//!   (no amounts ⇒ not in denominator). Other wrappers
//!   (`NEWT`/`CORR`/`TRDU`) with all 6 amounts `None` are also
//!   excluded — the indicator measures partial reporting, not
//!   missing reporting (that's covered by the granular
//!   `SFTR.MAR.*` checks added in A5).
//! - **Numerator:** records where exactly one of the two sides
//!   (posted, received) has any amount populated — the other
//!   side is entirely `None`. A symmetric "both sides
//!   populated" record is **not** counted.
//! - **Dimension:** completeness.
//!
//! Note: SFTR MAR is event-driven (per CP-pair, per event date)
//! and indexed by `collateral_portfolio_code`. The evidence's
//! `uti` field carries the portfolio code (mirror of the auth.085
//! convention adopted in v0.17 T3 computers).

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, Regime, SftrMarginActivityRecord};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_MAR_PARTIAL_SIDES_SFTR";
const DESCRIPTION: &str = "Share of SFTR MAR events (auth.070) that report only one of the two \
sides (posted xor received). Both-sides-populated is the expected reporting shape per \
RTS 2019/356; one-sided reporting is a completeness defect that obscures the symmetric \
margin obligation between CCP-cleared counterparties.";

#[inline]
fn has_any_posted(r: &SftrMarginActivityRecord) -> bool {
    r.initial_margin_posted.is_some()
        || r.variation_margin_posted.is_some()
        || r.excess_collateral_posted.is_some()
}

#[inline]
fn has_any_received(r: &SftrMarginActivityRecord) -> bool {
    r.initial_margin_received.is_some()
        || r.variation_margin_received.is_some()
        || r.excess_collateral_received.is_some()
}

/// Compute `DQI_MAR_PARTIAL_SIDES_SFTR`.
pub fn compute_dqi_mar_partial_sides_sftr(
    mar: &[SftrMarginActivityRecord],
    thresholds: &Thresholds,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    let mut denominator: u64 = 0;
    let mut numerator: u64 = 0;
    let mut offenders: Vec<DqiEvidence> = Vec::new();

    for r in mar {
        let posted = has_any_posted(r);
        let received = has_any_received(r);
        if !posted && !received {
            continue; // both empty (Err wrapper, or rare no-amount NEWT) — excluded.
        }
        denominator += 1;
        if posted == received {
            continue; // both populated — not a partial-sides case.
        }
        numerator += 1;
        let portfolio = r
            .collateral_portfolio_code
            .as_deref()
            .unwrap_or("<no-portfolio>");
        let which = if posted {
            "posted-only"
        } else {
            "received-only"
        };
        offenders.push(DqiEvidence {
            indicator_id: INDICATOR_ID.into(),
            uti: portfolio.to_owned(),
            counterparty: r.reporting_counterparty.clone(),
            asset_class: None,
            source_file: r.source_file.clone(),
            observed_value: Some(which.into()),
            explanation: format!(
                "MAR event reports {which} margin (the other side is entirely None)"
            ),
        });
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
        dimension: DqDimension::Completeness,
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
    use rust_decimal::Decimal;

    use super::*;
    use crate::dq::dqi::DqiStatus;

    fn rec(
        portfolio: &str,
        im_pst: Option<Decimal>,
        im_rcv: Option<Decimal>,
    ) -> SftrMarginActivityRecord {
        SftrMarginActivityRecord {
            collateral_portfolio_code: Some(portfolio.into()),
            initial_margin_posted: im_pst,
            initial_margin_received: im_rcv,
            ..Default::default()
        }
    }

    #[test]
    fn empty_is_not_applicable() {
        let (ind, _) = compute_dqi_mar_partial_sides_sftr(
            &[],
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn both_sides_populated_does_not_count() {
        let recs = vec![rec("P1", Some(Decimal::from(100)), Some(Decimal::from(80)))];
        let (ind, _) = compute_dqi_mar_partial_sides_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 1);
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn posted_only_record_fires_with_observed_value_tag() {
        let recs = vec![rec("P-POSTED", Some(Decimal::from(100)), None)];
        let (ind, ev) = compute_dqi_mar_partial_sides_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 1);
        assert_eq!(ind.numerator, 1);
        assert_eq!(ev.len(), 1);
        assert_eq!(ev[0].observed_value.as_deref(), Some("posted-only"));
    }

    #[test]
    fn received_only_record_fires_with_observed_value_tag() {
        let recs = vec![rec("P-RECEIVED", None, Some(Decimal::from(80)))];
        let (ind, ev) = compute_dqi_mar_partial_sides_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 1);
        assert_eq!(ev[0].observed_value.as_deref(), Some("received-only"));
    }

    #[test]
    fn err_wrapper_and_metadata_only_records_excluded_from_denominator() {
        // Mirror of the Err-wrapper (action_type=ERRT) case: no
        // amounts at all → excluded from denominator entirely.
        let r = SftrMarginActivityRecord {
            collateral_portfolio_code: Some("P-ERR".into()),
            action_type: Some("ERRT".into()),
            ..Default::default()
        };
        let (ind, _) = compute_dqi_mar_partial_sides_sftr(
            std::slice::from_ref(&r),
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 0);
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }
}
