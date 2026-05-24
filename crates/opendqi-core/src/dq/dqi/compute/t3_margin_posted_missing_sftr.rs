//! `DQI_T3_MARGIN_POSTED_MISSING_SFTR` — share of SFTR MSR
//! portfolios (auth.085 records) that report **no posted
//! margin** (neither IM nor VM nor excess collateral on the
//! posted side).
//!
//! - **Layer:** SFTR MSR (`auth.085`).
//! - **Denominator:** records with `collateral_portfolio_code`
//!   set (the only field marked `mandatory` in the
//!   `CollateralMarginNew10__1` element type — every well-
//!   formed Stat carries one).
//! - **Numerator:** records where all 3 posted amounts
//!   (`initial_margin_posted`, `variation_margin_posted`,
//!   `excess_collateral_posted`) are `None`.
//! - **Dimension:** completeness.
//!
//! Note: SFTR MSR is **portfolio-level**, indexed by
//! `collateral_portfolio_code` instead of UTI. The evidence's
//! `uti` field carries the portfolio code (the v1.0 Arrow
//! schema is frozen — `uti` is the canonical "primary record
//! key" column whose semantic adapts per indicator).

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, Regime, SftrMarginStateRecord};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_T3_MARGIN_POSTED_MISSING_SFTR";
const DESCRIPTION: &str = "Share of SFTR MSR portfolios (auth.085) that report no posted margin \
(initial_margin_posted, variation_margin_posted and excess_collateral_posted all None). \
Per ESMA RTS 2019/356, CCP-cleared SFTs must report margin posted at portfolio level.";

fn has_any_posted(r: &SftrMarginStateRecord) -> bool {
    r.initial_margin_posted.is_some()
        || r.variation_margin_posted.is_some()
        || r.excess_collateral_posted.is_some()
}

/// Compute `DQI_T3_MARGIN_POSTED_MISSING_SFTR`.
pub fn compute_dqi_t3_margin_posted_missing_sftr(
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
        if has_any_posted(r) {
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
            explanation: "no posted margin reported (IM/VM/excess-collateral all None)".into(),
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
        im_pst: Option<Decimal>,
        vm_pst: Option<Decimal>,
    ) -> SftrMarginStateRecord {
        SftrMarginStateRecord {
            collateral_portfolio_code: Some(portfolio.into()),
            initial_margin_posted: im_pst,
            variation_margin_posted: vm_pst,
            ..Default::default()
        }
    }

    #[test]
    fn empty_is_not_applicable() {
        let (ind, _) = compute_dqi_t3_margin_posted_missing_sftr(
            &[],
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn records_without_portfolio_excluded_from_denominator() {
        // No collateral_portfolio_code: should not count.
        let r = SftrMarginStateRecord {
            initial_margin_posted: Some(Decimal::from(100)),
            ..Default::default()
        };
        let (ind, _) = compute_dqi_t3_margin_posted_missing_sftr(
            std::slice::from_ref(&r),
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 0);
    }

    #[test]
    fn fully_posted_is_green() {
        let recs = vec![
            rec("P1", Some(Decimal::from(1000)), Some(Decimal::from(50))),
            rec("P2", Some(Decimal::from(500)), None), // IM-only still has posted
        ];
        let (ind, _) = compute_dqi_t3_margin_posted_missing_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 2);
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn no_posted_at_all_fires() {
        let recs = vec![
            rec("P1", None, None),                     // posted-missing
            rec("P2", Some(Decimal::from(100)), None), // posted
            rec("P3", None, None),                     // posted-missing
        ];
        let (ind, ev) = compute_dqi_t3_margin_posted_missing_sftr(
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
    fn excess_collateral_alone_is_still_posted() {
        // A portfolio reporting only XcssCollPstd (no IM/VM) is
        // still "posted" — the denominator excludes the missing-
        // posted bucket. Captures the edge case.
        let r = SftrMarginStateRecord {
            collateral_portfolio_code: Some("P-XCSS".into()),
            excess_collateral_posted: Some(Decimal::from(100)),
            ..Default::default()
        };
        let (ind, _) = compute_dqi_t3_margin_posted_missing_sftr(
            std::slice::from_ref(&r),
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 0);
    }
}
