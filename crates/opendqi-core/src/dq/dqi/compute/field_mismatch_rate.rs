//! `DQI_FIELD_MISMATCH_RATE` — share of per-trade
//! reconciliation records (paired with the counterparty) that
//! have at least one mismatched matching-criterion.
//!
//! - **Layer:** auth.091 per-tx reconciliation records.
//! - **Denominator:** records with `pairing_status` populated
//!   (excluding those whose status is unknown).
//! - **Numerator:** records where `mismatched_fields` is
//!   non-empty.
//! - **Dimension:** consistency.
//!
//! Generic "any criterion mismatch" rollup. The
//! per-criterion DQIs (`DQI_NOTIONAL_INCONSISTENT`,
//! `DQI_MARGIN_INCONSISTENT_PRE_HAIRCUT`,
//! `DQI_MARGIN_INCONSISTENT_POST_HAIRCUT`) zoom into
//! specific criterion families.

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, ReconciliationRecord, Regime};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_FIELD_MISMATCH_RATE";
const DESCRIPTION: &str = "Share of reconciliation records with at least one mismatched \
matching-criterion (from auth.091 MtchgCrit). Per-criterion breakdowns: DQI_NOTIONAL_INCONSISTENT \
+ DQI_MARGIN_INCONSISTENT_PRE/POST_HAIRCUT.";

/// Compute `DQI_FIELD_MISMATCH_RATE`.
pub fn compute_dqi_field_mismatch_rate(
    recon_records: &[ReconciliationRecord],
    thresholds: &Thresholds,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    let mut denominator: u64 = 0;
    let mut numerator: u64 = 0;
    let mut offenders: Vec<DqiEvidence> = Vec::new();

    for r in recon_records {
        if r.pairing_status.is_none() {
            continue;
        }
        denominator += 1;
        if r.mismatched_fields.is_empty() {
            continue;
        }
        numerator += 1;
        offenders.push(DqiEvidence {
            indicator_id: INDICATOR_ID.into(),
            uti: r.uti.clone().unwrap_or_default(),
            counterparty: r.reporting_counterparty.clone(),
            asset_class: None,
            source_file: r.source_file.clone(),
            observed_value: Some(r.mismatched_fields.join(",")),
            explanation: format!(
                "{} criterion(s) mismatched between counterparties",
                r.mismatched_fields.len()
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
        dimension: DqDimension::Consistency,
        table_scope: "auth.091".into(),
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

    fn rec(uti: &str, status: Option<&str>, mismatched: Vec<&str>) -> ReconciliationRecord {
        ReconciliationRecord {
            uti: Some(uti.into()),
            pairing_status: status.map(|s| s.into()),
            mismatched_fields: mismatched.into_iter().map(String::from).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn empty_input_is_not_applicable() {
        let (ind, _) = compute_dqi_field_mismatch_rate(
            &[],
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn all_clean_is_green() {
        let recs = vec![
            rec("U1", Some("PAIRED"), vec![]),
            rec("U2", Some("PAIRED"), vec![]),
        ];
        let (ind, _) = compute_dqi_field_mismatch_rate(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn any_mismatch_counts() {
        let recs = vec![
            rec("U1", Some("PAIRED"), vec!["NtnlAmt"]),
            rec("U2", Some("PAIRED"), vec!["CtrctTp", "CtrctVal"]),
            rec("U3", Some("PAIRED"), vec![]),
        ];
        let (ind, ev) = compute_dqi_field_mismatch_rate(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 3);
        assert_eq!(ind.numerator, 2);
        assert_eq!(ev.len(), 2);
        assert_eq!(ind.status, DqiStatus::Red);
    }
}
