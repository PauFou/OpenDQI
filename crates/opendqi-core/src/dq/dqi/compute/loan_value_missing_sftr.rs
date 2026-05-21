//! `DQI_LOAN_VALUE_MISSING_SFTR` — share of outstanding SFTR
//! TSR records that have no loan_value reported (or zero,
//! treated as not reported).
//!
//! SFTR mirror of EMIR's [`crate::dq::dqi::compute_dqi_val_missing`].
//!
//! - **Layer:** SFTR TSR (`auth.079`).
//! - **Denominator:** outstanding [`crate::SftrTrStateRecord`]
//!   (status not MATUR* / TERMIN*, termination_date unset).
//! - **Numerator:** outstanding records where `loan_value` is
//!   `None` or 0.
//! - **Dimension:** completeness.

use rust_decimal::Decimal;

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, Regime, SftrTrStateRecord};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_LOAN_VALUE_MISSING_SFTR";
const DESCRIPTION: &str = "Outstanding SFTR TSR records with no loan_value reported \
(or zero, treated as not reported). SFTR mirror of DQI_VAL_MISSING.";

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

/// Compute `DQI_LOAN_VALUE_MISSING_SFTR`.
pub fn compute_dqi_loan_value_missing_sftr(
    tsr: &[SftrTrStateRecord],
    thresholds: &Thresholds,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    let mut denominator: u64 = 0;
    let mut numerator: u64 = 0;
    let mut offenders: Vec<DqiEvidence> = Vec::new();

    for r in tsr {
        if !is_outstanding(r) {
            continue;
        }
        denominator += 1;
        let missing = r
            .loan_value
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
            observed_value: r.loan_value.as_ref().map(|d| d.to_string()),
            explanation: "loan_value is missing or zero on an outstanding SFT".into(),
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

    fn rec(uti: &str, status: Option<&str>, val: Option<Decimal>) -> SftrTrStateRecord {
        SftrTrStateRecord {
            uti: Some(uti.into()),
            status: status.map(|s| s.into()),
            loan_value: val,
            ..Default::default()
        }
    }

    #[test]
    fn empty_is_not_applicable() {
        let (ind, _) = compute_dqi_loan_value_missing_sftr(
            &[],
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn matured_excluded() {
        let recs = vec![rec("U1", Some("MATURED"), None)];
        let (ind, _) = compute_dqi_loan_value_missing_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 0);
    }

    #[test]
    fn outstanding_with_loan_is_green() {
        let recs = vec![
            rec("U1", Some("OUTSTANDING"), Some(Decimal::from(1_000_000))),
            rec("U2", Some("OUTSTANDING"), Some(Decimal::from(500_000))),
        ];
        let (ind, _) = compute_dqi_loan_value_missing_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn outstanding_no_loan_fires() {
        let recs = vec![
            rec("U1", Some("OUTSTANDING"), None),
            rec("U2", Some("OUTSTANDING"), Some(Decimal::ZERO)),
            rec("U3", Some("OUTSTANDING"), Some(Decimal::from(1_000))),
        ];
        let (ind, ev) = compute_dqi_loan_value_missing_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 3);
        assert_eq!(ind.numerator, 2);
        assert_eq!(ev.len(), 2);
    }
}
