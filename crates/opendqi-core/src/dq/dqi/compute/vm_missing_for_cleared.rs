//! `DQI_VM_MISSING_FOR_CLEARED` — share of fully-collateralised
//! MSR records (`collateralization_category = FCOL`) that report
//! no variation margin (posted or collected).
//!
//! Cleared trades MUST exchange variation margin per EMIR Article
//! 11 / RTS 2016/2251. The FCOL collateralisation category is the
//! closest proxy in the MSR for "this trade carries a margining
//! obligation" — when VM is then absent, it's a strong signal of
//! either a margining gap or a misreporting.
//!
//! More specific than [`crate::dq::dqi::compute_dqi_col_missing_state`]
//! (which checks the broad "is there an MSR row at all"). This
//! one zooms into a specific margin field being absent on rows
//! that should carry it.
//!
//! - **Layer:** MSR.
//! - **Denominator:** MSR records with `collateralization_category`
//!   = `FCOL` (fully-collateralised).
//! - **Numerator:** denominator records where BOTH VM fields
//!   (posted + collected) are `None` or zero.
//! - **Dimension:** completeness.

use rust_decimal::Decimal;

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, MarginStateRecord, Regime};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_VM_MISSING_FOR_CLEARED";
const DESCRIPTION: &str = "Share of fully-collateralised (FCOL) MSR records reporting no \
variation margin (posted + collected both absent or zero). Cleared/fully-collateralised trades \
MUST exchange VM per EMIR Art.11 / RTS 2016/2251.";

fn is_zero_or_missing(amount: &Option<Decimal>) -> bool {
    match amount {
        None => true,
        Some(d) => *d == Decimal::ZERO,
    }
}

/// Compute `DQI_VM_MISSING_FOR_CLEARED`.
pub fn compute_dqi_vm_missing_for_cleared(
    msr: &[MarginStateRecord],
    thresholds: &Thresholds,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    let mut denominator: u64 = 0;
    let mut numerator: u64 = 0;
    let mut offenders: Vec<DqiEvidence> = Vec::new();

    for r in msr {
        let is_cleared = r
            .collateralization_category
            .as_deref()
            .map(|s| s.eq_ignore_ascii_case("FCOL"))
            .unwrap_or(false);
        if !is_cleared {
            continue;
        }
        denominator += 1;
        let vm_missing = is_zero_or_missing(&r.variation_margin_posted_current)
            && is_zero_or_missing(&r.variation_margin_collected_current);
        if !vm_missing {
            continue;
        }
        numerator += 1;
        offenders.push(DqiEvidence {
            indicator_id: INDICATOR_ID.into(),
            uti: r.uti.clone().unwrap_or_default(),
            counterparty: r.counterparty_1.clone(),
            asset_class: None,
            source_file: r.source_file.clone(),
            observed_value: r.collateralization_category.clone(),
            explanation: "FCOL trade has no variation margin reported (posted + collected both \
                          absent or zero)"
                .into(),
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
        regime: Regime::Emir,
        dimension: DqDimension::Completeness,
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
    use super::*;
    use crate::dq::dqi::DqiStatus;

    fn rec(
        uti: &str,
        cat: Option<&str>,
        vm_posted: Option<i64>,
        vm_coll: Option<i64>,
    ) -> MarginStateRecord {
        MarginStateRecord {
            uti: Some(uti.into()),
            collateralization_category: cat.map(|s| s.into()),
            variation_margin_posted_current: vm_posted.map(Decimal::from),
            variation_margin_collected_current: vm_coll.map(Decimal::from),
            ..Default::default()
        }
    }

    #[test]
    fn empty_is_not_applicable() {
        let (ind, _) = compute_dqi_vm_missing_for_cleared(
            &[],
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn non_fcol_excluded_from_denominator() {
        let recs = vec![rec("U1", Some("UCOL"), None, None)];
        let (ind, _) = compute_dqi_vm_missing_for_cleared(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 0);
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn fcol_with_vm_is_green() {
        let recs = vec![rec("U1", Some("FCOL"), Some(1000), Some(900))];
        let (ind, _) = compute_dqi_vm_missing_for_cleared(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.denominator, 1);
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn fcol_with_no_vm_fires() {
        let recs = vec![
            rec("U1", Some("FCOL"), None, None),
            rec("U2", Some("FCOL"), Some(1000), None),
        ];
        let (ind, ev) = compute_dqi_vm_missing_for_cleared(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 2);
        assert_eq!(ind.numerator, 1);
        assert_eq!(ev[0].uti, "U1");
    }

    #[test]
    fn fcol_case_insensitive() {
        let recs = vec![rec("U1", Some("fcol"), None, None)];
        let (ind, _) = compute_dqi_vm_missing_for_cleared(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 1);
        assert_eq!(ind.numerator, 1);
    }
}
