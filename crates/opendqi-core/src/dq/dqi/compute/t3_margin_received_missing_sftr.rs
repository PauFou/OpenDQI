//! `DQI_T3_MARGIN_RECEIVED_MISSING_SFTR` — share of SFTR MSR
//! portfolios that report **no received margin** (neither IM
//! nor VM nor excess collateral on the received side).
//!
//! Symmetric to [`DQI_T3_MARGIN_POSTED_MISSING_SFTR`]
//! ([`super::t3_margin_posted_missing_sftr`]); same denominator
//! semantics (records with `collateral_portfolio_code` set);
//! numerator flips to the received side.
//!
//! - **Layer:** SFTR MSR (`auth.085`).
//! - **Denominator:** records with `collateral_portfolio_code` set.
//! - **Numerator:** records where all 3 received amounts
//!   (`initial_margin_received`, `variation_margin_received`,
//!   `excess_collateral_received`) are `None`.
//! - **Dimension:** completeness.

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, Regime, SftrMarginStateRecord};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_T3_MARGIN_RECEIVED_MISSING_SFTR";
const DESCRIPTION: &str = "Share of SFTR MSR portfolios (auth.085) that report no received margin \
(initial_margin_received, variation_margin_received and excess_collateral_received all None). \
Per ESMA RTS 2019/356, CCP-cleared SFTs must report margin received at portfolio level.";

fn has_any_received(r: &SftrMarginStateRecord) -> bool {
    r.initial_margin_received.is_some()
        || r.variation_margin_received.is_some()
        || r.excess_collateral_received.is_some()
}

/// Compute `DQI_T3_MARGIN_RECEIVED_MISSING_SFTR`.
pub fn compute_dqi_t3_margin_received_missing_sftr(
    msr: &[SftrMarginStateRecord],
    thresholds: &Thresholds,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    let mut denominator: u64 = 0;
    let mut numerator: u64 = 0;
    let mut offenders: Vec<DqiEvidence> = Vec::new();

    for r in msr {
        let Some(portfolio) = r.collateral_portfolio_code.as_deref() else {
            continue;
        };
        denominator += 1;
        if has_any_received(r) {
            continue;
        }
        numerator += 1;
        offenders.push(DqiEvidence {
            indicator_id: INDICATOR_ID.into(),
            uti: portfolio.to_owned(),
            counterparty: r.reporting_counterparty.clone(),
            asset_class: None,
            source_file: r.source_file.clone(),
            observed_value: None,
            explanation: "no received margin reported (IM/VM/excess-collateral all None)".into(),
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
    use rust_decimal::Decimal;

    use super::*;
    use crate::dq::dqi::DqiStatus;

    fn rec(
        portfolio: &str,
        im_rcv: Option<Decimal>,
        vm_rcv: Option<Decimal>,
    ) -> SftrMarginStateRecord {
        SftrMarginStateRecord {
            collateral_portfolio_code: Some(portfolio.into()),
            initial_margin_received: im_rcv,
            variation_margin_received: vm_rcv,
            ..Default::default()
        }
    }

    #[test]
    fn empty_is_not_applicable() {
        let (ind, _) = compute_dqi_t3_margin_received_missing_sftr(
            &[],
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn fully_received_is_green() {
        let recs = vec![rec(
            "P1",
            Some(Decimal::from(1000)),
            Some(Decimal::from(50)),
        )];
        let (ind, _) = compute_dqi_t3_margin_received_missing_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn no_received_fires() {
        let recs = vec![
            rec("P1", None, None),
            rec("P2", Some(Decimal::from(100)), None),
            rec("P3", None, None),
        ];
        let (ind, ev) = compute_dqi_t3_margin_received_missing_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 3);
        assert_eq!(ind.numerator, 2);
        assert_eq!(ev.len(), 2);
    }

    #[test]
    fn posted_only_record_still_fires_received_missing() {
        // A portfolio with PstdMrgnOrColl populated but
        // RcvdMrgnOrColl absent (XSD allows [0..1] on both)
        // — exactly the v0.17 A2' "posted-only" fixture case.
        let r = SftrMarginStateRecord {
            collateral_portfolio_code: Some("P-POSTED-ONLY".into()),
            initial_margin_posted: Some(Decimal::from(1000)),
            variation_margin_posted: Some(Decimal::from(50)),
            ..Default::default()
        };
        let (ind, _) = compute_dqi_t3_margin_received_missing_sftr(
            std::slice::from_ref(&r),
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 1);
    }
}
