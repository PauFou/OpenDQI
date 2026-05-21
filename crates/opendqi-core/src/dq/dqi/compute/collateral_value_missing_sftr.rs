//! `DQI_COLLATERAL_VALUE_MISSING_SFTR` — share of outstanding
//! SFTR TSR records that should carry collateral (portfolio
//! code set OR collateral_isin set) but report no
//! collateral_value.
//!
//! - **Layer:** SFTR TSR (`auth.079`).
//! - **Denominator:** outstanding SFTR records flagged as
//!   collateralised (collateral_portfolio_code OR
//!   collateral_isin populated).
//! - **Numerator:** denominator records with `collateral_value`
//!   missing or zero.
//! - **Dimension:** completeness.

use rust_decimal::Decimal;

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, Regime, SftrTrStateRecord};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_COLLATERAL_VALUE_MISSING_SFTR";
const DESCRIPTION: &str = "Outstanding collateralised SFTR records (collateral_portfolio_code OR \
collateral_isin set) with no collateral_value reported (or zero, treated as not reported).";

fn is_outstanding(r: &SftrTrStateRecord) -> bool {
    if r.termination_date.is_some() {
        return false;
    }
    match r.status.as_deref() {
        Some(s) => {
            let up = s.trim().to_ascii_uppercase();
            !(up.starts_with("MATUR") || up.starts_with("TERMIN"))
        }
        None => true,
    }
}

fn is_collateralised(r: &SftrTrStateRecord) -> bool {
    r.collateral_portfolio_code.is_some() || r.collateral_isin.is_some()
}

/// Compute `DQI_COLLATERAL_VALUE_MISSING_SFTR`.
pub fn compute_dqi_collateral_value_missing_sftr(
    tsr: &[SftrTrStateRecord],
    thresholds: &Thresholds,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    let mut denominator: u64 = 0;
    let mut numerator: u64 = 0;
    let mut offenders: Vec<DqiEvidence> = Vec::new();

    for r in tsr {
        if !is_outstanding(r) || !is_collateralised(r) {
            continue;
        }
        denominator += 1;
        let missing = r
            .collateral_value
            .as_ref()
            .map(|d| *d == Decimal::ZERO)
            .unwrap_or(true);
        if !missing {
            continue;
        }
        numerator += 1;
        offenders.push(DqiEvidence {
            indicator_id: INDICATOR_ID.into(),
            uti: r.uti.clone().unwrap_or_default(),
            counterparty: r.reporting_counterparty.clone(),
            asset_class: r.sft_type.clone(),
            source_file: r.source_file.clone(),
            observed_value: r.collateral_isin.clone(),
            explanation: "outstanding collateralised SFT has no collateral_value reported".into(),
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

    fn rec(uti: &str, portfolio: Option<&str>, coll_val: Option<Decimal>) -> SftrTrStateRecord {
        SftrTrStateRecord {
            uti: Some(uti.into()),
            status: Some("OUTSTANDING".into()),
            collateral_portfolio_code: portfolio.map(|s| s.into()),
            collateral_value: coll_val,
            ..Default::default()
        }
    }

    #[test]
    fn empty_is_not_applicable() {
        let (ind, _) = compute_dqi_collateral_value_missing_sftr(
            &[],
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn uncollateralised_excluded() {
        let recs = vec![rec("U1", None, None)];
        let (ind, _) = compute_dqi_collateral_value_missing_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 0);
    }

    #[test]
    fn collateralised_with_value_is_green() {
        let recs = vec![rec("U1", Some("PF-1"), Some(Decimal::from(100_000)))];
        let (ind, _) = compute_dqi_collateral_value_missing_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn collateralised_no_value_fires() {
        let recs = vec![
            rec("U1", Some("PF-1"), None),
            rec("U2", Some("PF-2"), Some(Decimal::ZERO)),
            rec("U3", Some("PF-3"), Some(Decimal::from(1_000))),
        ];
        let (ind, _) = compute_dqi_collateral_value_missing_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 3);
        assert_eq!(ind.numerator, 2);
    }
}
