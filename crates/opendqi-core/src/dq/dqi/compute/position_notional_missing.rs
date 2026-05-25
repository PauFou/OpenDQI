//! `DQI_POSITION_NOTIONAL_MISSING` — share of PosSet / CcyPosSet
//! records that report no notional amount.
//!
//! - **Layer:** EMIR Position Set (`auth.090`).
//! - **Denominator:** records whose `position_set_kind` is
//!   `PosSet` or `CcyPosSet`. CollPosSet / CcyCollPosSet kinds
//!   don't carry the `Ttl/Buyr/Ntnl/Amt` path at the XSD level
//!   and are excluded.
//! - **Numerator:** records with `notional` is `None`.
//! - **Dimension:** completeness.

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, EmirPositionSetRecord, Regime};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_POSITION_NOTIONAL_MISSING";
const DESCRIPTION: &str = "Share of EMIR auth.090 PosSet/CcyPosSet records that report no \
notional amount (Mtrcs/Ttl/Buyr/Ntnl/Amt absent). CollPosSet/CcyCollPosSet records are \
excluded — they don't carry the notional metric at the XSD level.";

fn is_pos_set_kind(r: &EmirPositionSetRecord) -> bool {
    matches!(
        r.position_set_kind.as_deref(),
        Some("PosSet") | Some("CcyPosSet")
    )
}

/// Compute `DQI_POSITION_NOTIONAL_MISSING`.
pub fn compute_dqi_position_notional_missing(
    positions: &[EmirPositionSetRecord],
    thresholds: &Thresholds,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    let mut denominator: u64 = 0;
    let mut numerator: u64 = 0;
    let mut offenders: Vec<DqiEvidence> = Vec::new();

    for r in positions {
        if !is_pos_set_kind(r) {
            continue;
        }
        denominator += 1;
        if r.notional.is_some() {
            continue;
        }
        numerator += 1;
        let label = r.record_id.as_deref().unwrap_or("<no-record-id>");
        offenders.push(DqiEvidence {
            indicator_id: INDICATOR_ID.into(),
            uti: label.to_owned(),
            counterparty: r.reporting_counterparty.clone(),
            asset_class: r.asset_class.clone(),
            source_file: r.source_file.clone(),
            observed_value: r.position_set_kind.clone(),
            explanation: format!(
                "{} record carries no notional amount (Ttl/Buyr/Ntnl/Amt absent)",
                r.position_set_kind.as_deref().unwrap_or("?")
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
        regime: Regime::Emir,
        dimension: DqDimension::Completeness,
        table_scope: "EMIR-POS".into(),
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

    fn rec(kind: &str, notional: Option<Decimal>) -> EmirPositionSetRecord {
        EmirPositionSetRecord {
            record_id: Some(format!("R-{kind}")),
            position_set_kind: Some(kind.into()),
            notional,
            ..Default::default()
        }
    }

    #[test]
    fn empty_is_not_applicable() {
        let (ind, _) = compute_dqi_position_notional_missing(
            &[],
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn collposset_kinds_excluded_from_denominator() {
        let recs = vec![rec("CollPosSet", None), rec("CcyCollPosSet", None)];
        let (ind, _) = compute_dqi_position_notional_missing(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 0);
    }

    #[test]
    fn posset_with_notional_is_green() {
        let recs = vec![
            rec("PosSet", Some(Decimal::from(100))),
            rec("CcyPosSet", Some(Decimal::from(200))),
        ];
        let (ind, _) = compute_dqi_position_notional_missing(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 2);
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn posset_without_notional_fires_with_evidence() {
        let recs = vec![
            rec("PosSet", Some(Decimal::from(100))),
            rec("PosSet", None),
            rec("CcyPosSet", None),
            rec("CollPosSet", None), // excluded
        ];
        let (ind, ev) = compute_dqi_position_notional_missing(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 3);
        assert_eq!(ind.numerator, 2);
        assert_eq!(ev.len(), 2);
    }
}
