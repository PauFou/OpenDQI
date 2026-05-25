//! `DQI_REUSE_ERR_RETRACTION_RATE_SFTR` — share of SFTR auth.071
//! records that are `Err`-wrapper retractions.
//!
//! - **Layer:** SFTR Reuse Activity (`auth.071`).
//! - **Denominator:** every record in the batch (no exclusions).
//! - **Numerator:** records whose `action_type` is `"ERRT"`
//!   (the canonical code derived from the auth.071 `Err`
//!   wrapper in `ReuseDataReport6Choice__1`).
//! - **Dimension:** timeliness (operational signal).
//!
//! Rationale: `Err` wrappers are used to retract a previously
//! submitted reuse report (the contract never came into
//! existence or was reported by mistake). A high `Err` rate
//! indicates poor first-shot quality on the NEWT/CRUD side —
//! the firm submits then promptly retracts. The threshold
//! pair is calibrated against the existing reconciliation
//! tolerances (5 % amber / 20 % red): more than 1 record in 20
//! being a retraction signals systemic upstream reporting
//! defects worth investigating.

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, Regime, SftrReuseActivityRecord};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_REUSE_ERR_RETRACTION_RATE_SFTR";
const DESCRIPTION: &str = "Share of SFTR auth.071 records that are Err-wrapper retractions \
(action_type=ERRT). High rates suggest poor first-shot reporting quality on the NEWT/CRUD side \
— the firm submits then retracts. Operational signal threshold pair mirrors the SFTR \
reconciliation tolerances.";

/// Compute `DQI_REUSE_ERR_RETRACTION_RATE_SFTR`.
pub fn compute_dqi_reuse_err_retraction_rate_sftr(
    reuse: &[SftrReuseActivityRecord],
    thresholds: &Thresholds,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    let mut denominator: u64 = 0;
    let mut numerator: u64 = 0;
    let mut offenders: Vec<DqiEvidence> = Vec::new();

    for r in reuse {
        denominator += 1;
        if r.action_type.as_deref() == Some("ERRT") {
            numerator += 1;
            let label = r.record_id.as_deref().unwrap_or("<no-record-id>");
            offenders.push(DqiEvidence {
                indicator_id: INDICATOR_ID.into(),
                uti: label.to_owned(),
                counterparty: r.reporting_counterparty.clone(),
                asset_class: None,
                source_file: r.source_file.clone(),
                observed_value: Some("ERRT".into()),
                explanation: "reuse retraction (Err wrapper) — previously submitted record \
                              cancelled"
                    .into(),
            });
        }
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
        dimension: DqDimension::Timeliness,
        table_scope: "SFTR-REU".into(),
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

    fn rec(id: &str, action: &str) -> SftrReuseActivityRecord {
        SftrReuseActivityRecord {
            record_id: Some(id.into()),
            action_type: Some(action.into()),
            ..Default::default()
        }
    }

    #[test]
    fn empty_is_not_applicable() {
        let (ind, _) = compute_dqi_reuse_err_retraction_rate_sftr(
            &[],
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn no_err_records_is_green() {
        let recs = vec![rec("R1", "NEWT"), rec("R2", "CORR"), rec("R3", "CRUD")];
        let (ind, _) = compute_dqi_reuse_err_retraction_rate_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 3);
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn one_err_in_twenty_is_below_amber() {
        // 19 NEWT + 1 ERRT = 5 % retraction → at the amber
        // boundary inclusive → still green per the standard
        // boundary rules.
        let mut recs: Vec<SftrReuseActivityRecord> =
            (0..19).map(|i| rec(&format!("R{i}"), "NEWT")).collect();
        recs.push(rec("R-ERR", "ERRT"));
        let (ind, _) = compute_dqi_reuse_err_retraction_rate_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 1);
        assert_eq!(ind.denominator, 20);
        assert_eq!(ind.status, DqiStatus::Green); // 0.05 ≤ amber 0.05
    }

    #[test]
    fn high_retraction_rate_is_red_with_evidence() {
        // 6 NEWT + 4 ERRT = 40 % retraction → red.
        let mut recs: Vec<SftrReuseActivityRecord> =
            (0..6).map(|i| rec(&format!("OK-{i}"), "NEWT")).collect();
        for i in 0..4 {
            recs.push(rec(&format!("RETRACT-{i}"), "ERRT"));
        }
        let (ind, ev) = compute_dqi_reuse_err_retraction_rate_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 4);
        assert_eq!(ind.denominator, 10);
        assert_eq!(ind.status, DqiStatus::Red);
        assert_eq!(ev.len(), 4);
        for e in &ev {
            assert!(e.uti.starts_with("RETRACT-"));
            assert_eq!(e.observed_value.as_deref(), Some("ERRT"));
        }
    }

    #[test]
    fn unknown_action_type_does_not_count_as_err() {
        // Only literal "ERRT" counts. A None or BOGUS action
        // type is invalid (SFTR.REU.* granular check territory)
        // but doesn't inflate the retraction rate.
        let recs = vec![
            rec("R1", "NEWT"),
            SftrReuseActivityRecord {
                record_id: Some("R2".into()),
                action_type: None,
                ..Default::default()
            },
            SftrReuseActivityRecord {
                record_id: Some("R3".into()),
                action_type: Some("BOGUS".into()),
                ..Default::default()
            },
        ];
        let (ind, _) = compute_dqi_reuse_err_retraction_rate_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.denominator, 3);
    }
}
