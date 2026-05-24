//! `DQI_UNDER_COLLATERALIZATION_SFTR` — share of SFTR TSR
//! records where the **post-haircut collateral value** is
//! strictly less than the loan value, i.e. the collateral
//! after haircut application does not cover the loan.
//!
//! - **Layer:** SFTR TSR (`auth.079`).
//! - **Denominator:** records with all 3 inputs set :
//!   `loan_value`, `collateral_value`, `haircut`.
//! - **Numerator:** records where
//!   `collateral_value * (1 - haircut) < loan_value`.
//! - **Dimension:** accuracy.
//!
//! This is a simple SFT-style collateral-adequacy check : the
//! collateral, once haircut is applied, should cover the loan.
//! When it doesn't, either :
//! - the values are mis-reported (a DQ defect — what this DQI
//!   surfaces), or
//! - the SFT is genuinely under-collateralised (a credit-risk
//!   matter outside OpenDQI's scope, but worth flagging
//!   nonetheless).
//!
//! Records missing any of the 3 inputs are excluded from the
//! denominator (the granular SFTR.COMP.{LOAN,COLLATERAL,HAIRCUT}
//! _MISSING checks cover those gaps).

use rust_decimal::Decimal;

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, Regime, SftrTrStateRecord};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_UNDER_COLLATERALIZATION_SFTR";
const DESCRIPTION: &str = "Share of SFTR TSR records where collateral_value × (1 − haircut) is \
strictly less than loan_value. Counts only records with all 3 inputs populated. Flags either \
mis-reporting (DQ defect) or genuine under-collateralisation (credit-risk signal).";

/// Compute `DQI_UNDER_COLLATERALIZATION_SFTR`.
pub fn compute_dqi_under_collateralization_sftr(
    tsr: &[SftrTrStateRecord],
    thresholds: &Thresholds,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    let mut denominator: u64 = 0;
    let mut numerator: u64 = 0;
    let mut offenders: Vec<DqiEvidence> = Vec::new();

    for r in tsr {
        let (Some(loan), Some(coll), Some(haircut)) = (r.loan_value, r.collateral_value, r.haircut)
        else {
            continue;
        };
        denominator += 1;
        let effective_collateral = coll * (Decimal::ONE - haircut);
        if effective_collateral >= loan {
            continue;
        }
        numerator += 1;
        let shortfall = loan - effective_collateral;
        offenders.push(DqiEvidence {
            indicator_id: INDICATOR_ID.into(),
            uti: r.uti.clone().unwrap_or_default(),
            counterparty: r.reporting_counterparty.clone(),
            asset_class: r.sft_type.clone(),
            source_file: r.source_file.clone(),
            observed_value: Some(format!(
                "loan={loan}, eff_coll={effective_collateral}, shortfall={shortfall}"
            )),
            explanation: "collateral_value × (1 − haircut) < loan_value".into(),
        });
    }

    // Worst (biggest shortfall) first.
    offenders.sort_by(|a, b| b.observed_value.cmp(&a.observed_value));
    offenders.truncate(EVIDENCE_TOP_N);

    let pair = resolve_threshold(thresholds, INDICATOR_ID);
    let (rate, status) = rate_with_status(numerator, denominator, &pair);
    let indicator = DqiIndicator {
        indicator_id: INDICATOR_ID.into(),
        regime: Regime::Sftr,
        dimension: DqDimension::Accuracy,
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
    use super::*;
    use crate::dq::dqi::DqiStatus;
    use std::str::FromStr;

    fn rec(
        uti: &str,
        loan: Option<Decimal>,
        coll: Option<Decimal>,
        haircut: Option<Decimal>,
    ) -> SftrTrStateRecord {
        SftrTrStateRecord {
            uti: Some(uti.into()),
            loan_value: loan,
            collateral_value: coll,
            haircut,
            ..Default::default()
        }
    }

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    #[test]
    fn empty_input_is_not_applicable() {
        let (ind, _) = compute_dqi_under_collateralization_sftr(
            &[],
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn missing_inputs_excluded_from_denominator() {
        let recs = vec![
            rec("U1", None, Some(d("100")), Some(d("0.05"))),
            rec("U2", Some(d("100")), None, Some(d("0.05"))),
            rec("U3", Some(d("100")), Some(d("100")), None),
        ];
        let (ind, _) = compute_dqi_under_collateralization_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 0);
    }

    #[test]
    fn over_collateralized_is_green() {
        // loan 100, coll 110, haircut 5% → effective 110*0.95 = 104.5 ≥ 100 → OK
        let recs = vec![rec("U1", Some(d("100")), Some(d("110")), Some(d("0.05")))];
        let (ind, _) = compute_dqi_under_collateralization_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn under_collateralized_fires() {
        // U1: loan 100, coll 100, haircut 5% → 95 < 100 → fires
        // U2: loan 100, coll 90, haircut 0% → 90 < 100 → fires
        // U3: loan 100, coll 200, haircut 50% → 100 = 100 → green (>= ok)
        let recs = vec![
            rec("U1", Some(d("100")), Some(d("100")), Some(d("0.05"))),
            rec("U2", Some(d("100")), Some(d("90")), Some(d("0.0"))),
            rec("U3", Some(d("100")), Some(d("200")), Some(d("0.5"))),
        ];
        let (ind, ev) = compute_dqi_under_collateralization_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 3);
        assert_eq!(ind.numerator, 2);
        assert_eq!(ev.len(), 2);
    }

    #[test]
    fn boundary_equal_is_green() {
        // loan 100, coll 100, haircut 0% → exactly 100, NOT under → green
        let recs = vec![rec("U1", Some(d("100")), Some(d("100")), Some(d("0.0")))];
        let (ind, _) = compute_dqi_under_collateralization_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 0);
    }
}
