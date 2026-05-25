//! `DQI_REUSE_VOLUME_MISSING_SFTR` — share of non-`Err` SFTR
//! auth.071 records that carry **no reuse content at all**
//! (neither security reuse nor cash reinvestment).
//!
//! - **Layer:** SFTR Reuse Activity (`auth.071`).
//! - **Denominator:** records whose `action_type` is `NEWT`,
//!   `CORR`, or `CRUD`. `ERRT` records are retractions that
//!   carry no content by XSD design and are excluded.
//! - **Numerator:** records where both `total_reuse_value` is
//!   `None` (no `Scty/ReuseVal` observed) AND
//!   `cash_reinvestment_rate` is `None` (no `Csh/CshRinvstmtRate`
//!   observed). The XSD makes `CollCmpnt` optional (`[0..1]`)
//!   so such "metadata-only" non-Err records are technically
//!   well-formed — but they defeat the purpose of the reuse
//!   report (no reuse content = nothing to track).
//! - **Dimension:** completeness.
//!
//! Honest design note: the original v0.18 plan listed a
//! `DQI_REUSE_VOLUME_RATE_SFTR` cross-referencing TSR UTIs
//! against the reuse activity report. XSD verification showed
//! auth.071 carries **no UTI cross-reference field** — records
//! are keyed by submitter + event day + ISIN. The proposed DQI
//! was undefined on the actual XSD shape and got revised to
//! this completeness indicator, which IS computable from the
//! shipped fields.

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, Regime, SftrReuseActivityRecord};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_REUSE_VOLUME_MISSING_SFTR";
const DESCRIPTION: &str = "Share of non-Err SFTR auth.071 records (NEWT/CORR/CRUD) that report \
neither security reuse (Scty/ReuseVal) nor cash reinvestment (Csh/CshRinvstmtRate). The XSD \
makes CollCmpnt optional so these records are well-formed, but a reuse report carrying no \
reuse content defeats the purpose of the message.";

fn has_reuse_content(r: &SftrReuseActivityRecord) -> bool {
    r.total_reuse_value.is_some() || r.cash_reinvestment_rate.is_some()
}

/// Compute `DQI_REUSE_VOLUME_MISSING_SFTR`.
pub fn compute_dqi_reuse_volume_missing_sftr(
    reuse: &[SftrReuseActivityRecord],
    thresholds: &Thresholds,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    let mut denominator: u64 = 0;
    let mut numerator: u64 = 0;
    let mut offenders: Vec<DqiEvidence> = Vec::new();

    for r in reuse {
        // Skip Err retractions — they have no content by XSD design.
        if r.action_type.as_deref() == Some("ERRT") {
            continue;
        }
        denominator += 1;
        if has_reuse_content(r) {
            continue;
        }
        numerator += 1;
        let label = r.record_id.as_deref().unwrap_or("<no-record-id>");
        offenders.push(DqiEvidence {
            indicator_id: INDICATOR_ID.into(),
            uti: label.to_owned(),
            counterparty: r.reporting_counterparty.clone(),
            asset_class: None,
            source_file: r.source_file.clone(),
            observed_value: r.action_type.clone(),
            explanation: "reuse report carries no Scty/ReuseVal and no Csh/CshRinvstmtRate".into(),
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
    use rust_decimal::Decimal;

    use super::*;
    use crate::dq::dqi::DqiStatus;

    fn rec(
        record_id: &str,
        action: &str,
        total_reuse_value: Option<Decimal>,
        cash_rate: Option<Decimal>,
    ) -> SftrReuseActivityRecord {
        SftrReuseActivityRecord {
            record_id: Some(record_id.into()),
            action_type: Some(action.into()),
            total_reuse_value,
            cash_reinvestment_rate: cash_rate,
            ..Default::default()
        }
    }

    #[test]
    fn empty_is_not_applicable() {
        let (ind, _) = compute_dqi_reuse_volume_missing_sftr(
            &[],
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn err_records_excluded_from_denominator() {
        // 3 ERR records → denominator stays 0 → NotApplicable.
        let recs = vec![
            rec("R1", "ERRT", None, None),
            rec("R2", "ERRT", None, None),
            rec("R3", "ERRT", None, None),
        ];
        let (ind, _) = compute_dqi_reuse_volume_missing_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 0);
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn record_with_only_total_reuse_value_does_not_fire() {
        let recs = vec![rec("R1", "NEWT", Some(Decimal::from(1000)), None)];
        let (ind, _) = compute_dqi_reuse_volume_missing_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 1);
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn record_with_only_cash_rate_does_not_fire() {
        let recs = vec![rec("R1", "CRUD", None, Some(Decimal::new(125, 4)))];
        let (ind, _) = compute_dqi_reuse_volume_missing_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 1);
        assert_eq!(ind.numerator, 0);
    }

    #[test]
    fn empty_metadata_record_fires_with_evidence() {
        let recs = vec![
            rec("R1", "NEWT", Some(Decimal::from(1000)), None), // valid
            rec("R2", "NEWT", None, None),                      // fires
            rec("R3", "CORR", None, None),                      // fires
            rec("R4", "ERRT", None, None),                      // skipped
        ];
        let (ind, ev) = compute_dqi_reuse_volume_missing_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 3); // R1, R2, R3 (R4 excluded)
        assert_eq!(ind.numerator, 2); // R2 + R3
        assert_eq!(ev.len(), 2);
        assert_eq!(ev[0].uti, "R2");
        assert_eq!(ev[1].uti, "R3");
        assert_eq!(ev[0].observed_value.as_deref(), Some("NEWT"));
    }
}
