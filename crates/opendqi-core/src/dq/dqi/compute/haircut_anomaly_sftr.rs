//! `DQI_HAIRCUT_ANOMALY_SFTR` — share of SFTR TSR records
//! whose `haircut` is outside the regulatory range `[0.0, 1.0]`
//! per ESMA RTS 2019/356 Art. 4 (Haircut bounds).
//!
//! - **Layer:** SFTR TSR (`auth.079`).
//! - **Denominator:** records with `haircut` set.
//! - **Numerator:** records where `haircut < 0` OR
//!   `haircut > 1` (strict). The configurable
//!   `DqiThresholdPair` controls amber/red on the rate,
//!   NOT the haircut bound itself (which is regulatory and
//!   fixed at `[0, 1]`).
//! - **Dimension:** accuracy.
//!
//! Complementary to the granular `SFTR.COMP.HAIRCUT_OUT_OF_RANGE`
//! check : this DQI rolls the per-record violations into a
//! single rate for committee reporting.

use rust_decimal::Decimal;

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, Regime, SftrTrStateRecord};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_HAIRCUT_ANOMALY_SFTR";
const DESCRIPTION: &str = "Share of SFTR TSR records with haircut outside the regulatory \
range [0.0, 1.0] (ESMA RTS 2019/356 Art. 4). Counts only records with haircut populated. \
Rolls up the granular SFTR.COMP.HAIRCUT_OUT_OF_RANGE per-record check.";

fn out_of_range(h: Decimal) -> bool {
    h < Decimal::ZERO || h > Decimal::ONE
}

/// Compute `DQI_HAIRCUT_ANOMALY_SFTR`.
pub fn compute_dqi_haircut_anomaly_sftr(
    tsr: &[SftrTrStateRecord],
    thresholds: &Thresholds,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    let mut denominator: u64 = 0;
    let mut numerator: u64 = 0;
    let mut offenders: Vec<DqiEvidence> = Vec::new();

    for r in tsr {
        let Some(h) = r.haircut else { continue };
        denominator += 1;
        if !out_of_range(h) {
            continue;
        }
        numerator += 1;
        offenders.push(DqiEvidence {
            indicator_id: INDICATOR_ID.into(),
            uti: r.uti.clone().unwrap_or_default(),
            counterparty: r.reporting_counterparty.clone(),
            asset_class: r.sft_type.clone(),
            source_file: r.source_file.clone(),
            observed_value: Some(h.to_string()),
            explanation: "haircut outside [0.0, 1.0] regulatory range".into(),
        });
    }

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

    fn rec(uti: &str, h: Option<Decimal>) -> SftrTrStateRecord {
        SftrTrStateRecord {
            uti: Some(uti.into()),
            haircut: h,
            ..Default::default()
        }
    }

    #[test]
    fn empty_input_is_not_applicable() {
        let (ind, _) = compute_dqi_haircut_anomaly_sftr(
            &[],
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn no_haircut_excluded() {
        let recs = vec![rec("U1", None)];
        let (ind, _) = compute_dqi_haircut_anomaly_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 0);
    }

    #[test]
    fn in_range_is_green() {
        let recs = vec![
            rec("U1", Some(Decimal::from_str("0.0").unwrap())),
            rec("U2", Some(Decimal::from_str("0.05").unwrap())),
            rec("U3", Some(Decimal::from_str("1.0").unwrap())),
        ];
        let (ind, _) = compute_dqi_haircut_anomaly_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn out_of_range_fires_on_both_sides() {
        let recs = vec![
            rec("U1", Some(Decimal::from_str("-0.01").unwrap())),
            rec("U2", Some(Decimal::from_str("1.01").unwrap())),
            rec("U3", Some(Decimal::from_str("0.5").unwrap())),
        ];
        let (ind, ev) = compute_dqi_haircut_anomaly_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 3);
        assert_eq!(ind.numerator, 2);
        assert_eq!(ev.len(), 2);
    }
}
