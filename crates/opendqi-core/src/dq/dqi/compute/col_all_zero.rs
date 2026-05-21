//! `DQI_COL_ALL_ZERO` — share of MSR records whose four margin
//! fields (IM posted / IM collected / VM posted / VM collected)
//! are **all** zero or NULL. Signals either a UCOL (uncollat.)
//! trade incorrectly reported, or a margining failure.
//!
//! - **Layer:** MSR (`auth.109`, [`crate::model::MarginStateRecord`]).
//! - **Denominator:** all MSR rows.
//! - **Numerator:** rows where every margin field is `None` or `0`.
//! - **Dimension:** accuracy.

use rust_decimal::Decimal;

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, MarginStateRecord, Regime};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_COL_ALL_ZERO";
const DESCRIPTION: &str = "MSR records whose four margin amounts (IM/VM × posted/collected) \
are all zero or NULL — likely UCOL trade incorrectly reported or margining gap.";

fn is_zero_or_missing(amount: &Option<Decimal>) -> bool {
    match amount {
        None => true,
        Some(d) => *d == Decimal::ZERO,
    }
}

/// Compute `DQI_COL_ALL_ZERO` on an MSR snapshot.
pub fn compute_dqi_col_all_zero(
    msr: &[MarginStateRecord],
    thresholds: &Thresholds,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    let mut denominator: u64 = 0;
    let mut numerator: u64 = 0;
    let mut offenders: Vec<DqiEvidence> = Vec::new();

    for r in msr {
        denominator += 1;
        let all_zero = is_zero_or_missing(&r.initial_margin_posted_current)
            && is_zero_or_missing(&r.initial_margin_collected_current)
            && is_zero_or_missing(&r.variation_margin_posted_current)
            && is_zero_or_missing(&r.variation_margin_collected_current);
        if !all_zero {
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
            explanation: "all four margin amounts are zero or NULL".into(),
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
        dimension: DqDimension::Accuracy,
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
        ima: Option<i64>,
        imb: Option<i64>,
        vma: Option<i64>,
        vmb: Option<i64>,
    ) -> MarginStateRecord {
        MarginStateRecord {
            uti: Some(uti.into()),
            initial_margin_posted_current: ima.map(Decimal::from),
            initial_margin_collected_current: imb.map(Decimal::from),
            variation_margin_posted_current: vma.map(Decimal::from),
            variation_margin_collected_current: vmb.map(Decimal::from),
            ..Default::default()
        }
    }

    #[test]
    fn empty_input_is_not_applicable() {
        let (ind, ev) =
            compute_dqi_col_all_zero(&[], &Thresholds::default(), MappingPresence::default());
        assert_eq!(ind.status, DqiStatus::NotApplicable);
        assert!(ev.is_empty());
    }

    #[test]
    fn all_zeros_breach() {
        let recs = vec![rec("U1", Some(0), Some(0), Some(0), Some(0))];
        let (ind, ev) =
            compute_dqi_col_all_zero(&recs, &Thresholds::default(), MappingPresence::default());
        assert_eq!(ind.numerator, 1);
        assert_eq!(ind.denominator, 1);
        assert_eq!(ev.len(), 1);
    }

    #[test]
    fn all_nulls_count_as_all_zero() {
        let recs = vec![rec("U1", None, None, None, None)];
        let (ind, _) =
            compute_dqi_col_all_zero(&recs, &Thresholds::default(), MappingPresence::default());
        assert_eq!(ind.numerator, 1);
    }

    #[test]
    fn mixed_nulls_and_zeros_count_as_all_zero() {
        let recs = vec![rec("U1", None, Some(0), None, Some(0))];
        let (ind, _) =
            compute_dqi_col_all_zero(&recs, &Thresholds::default(), MappingPresence::default());
        assert_eq!(ind.numerator, 1);
    }

    #[test]
    fn any_nonzero_excludes_record() {
        let recs = vec![rec("U1", Some(100), Some(0), None, None)];
        let (ind, _) =
            compute_dqi_col_all_zero(&recs, &Thresholds::default(), MappingPresence::default());
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.denominator, 1);
        assert_eq!(ind.status, DqiStatus::Green);
    }
}
