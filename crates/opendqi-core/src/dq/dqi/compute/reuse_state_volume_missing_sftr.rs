//! `DQI_REUSE_STATE_VOLUME_MISSING_SFTR` — share of SFTR
//! auth.086 state-snapshot records that report **no reuse
//! content at all** (neither security reuse nor cash
//! reinvestment).
//!
//! State-side mirror of `DQI_REUSE_VOLUME_MISSING_SFTR`
//! (auth.071 events). The semantics are slightly different:
//! - the auth.071 version excludes `Err` retractions because
//!   they're metadata-only by XSD design;
//! - the auth.086 version doesn't have action wrappers, so
//!   every `Stat` is considered — no exclusion.
//!
//! - **Layer:** SFTR Reuse State (`auth.086`).
//! - **Denominator:** every record in the input slice.
//! - **Numerator:** records where both `total_reuse_value` is
//!   `None` AND `cash_reinvestment_rate` is `None`. The XSD
//!   makes `CollCmpnt` optional (`[0..1]`) so such snapshots
//!   are well-formed, but a state report carrying no reuse
//!   content defeats the purpose of the message.
//! - **Dimension:** completeness.

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, Regime, SftrReuseStateRecord};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_REUSE_STATE_VOLUME_MISSING_SFTR";
const DESCRIPTION: &str = "Share of SFTR auth.086 state-snapshot records that report neither \
security reuse (Scty/ReuseVal) nor cash reinvestment (Csh/CshRinvstmtRate). The XSD makes \
CollCmpnt optional so these records are well-formed, but a state snapshot carrying no reuse \
content defeats the purpose of the message.";

fn has_reuse_content(r: &SftrReuseStateRecord) -> bool {
    r.total_reuse_value.is_some() || r.cash_reinvestment_rate.is_some()
}

/// Compute `DQI_REUSE_STATE_VOLUME_MISSING_SFTR`.
pub fn compute_dqi_reuse_state_volume_missing_sftr(
    reuse_state: &[SftrReuseStateRecord],
    thresholds: &Thresholds,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    let mut denominator: u64 = 0;
    let mut numerator: u64 = 0;
    let mut offenders: Vec<DqiEvidence> = Vec::new();

    for r in reuse_state {
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
            explanation: "state snapshot carries no Scty/ReuseVal and no Csh/CshRinvstmtRate"
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
        regime: Regime::Sftr,
        dimension: DqDimension::Completeness,
        table_scope: "SFTR-REU-STATE".into(),
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

    fn rec(id: &str, total: Option<Decimal>, rate: Option<Decimal>) -> SftrReuseStateRecord {
        SftrReuseStateRecord {
            record_id: Some(id.into()),
            total_reuse_value: total,
            cash_reinvestment_rate: rate,
            ..Default::default()
        }
    }

    #[test]
    fn empty_is_not_applicable() {
        let (ind, _) = compute_dqi_reuse_state_volume_missing_sftr(
            &[],
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn record_with_only_total_reuse_value_does_not_fire() {
        let recs = vec![rec("R1", Some(Decimal::from(1000)), None)];
        let (ind, _) = compute_dqi_reuse_state_volume_missing_sftr(
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
        let recs = vec![rec("R1", None, Some(Decimal::new(125, 4)))];
        let (ind, _) = compute_dqi_reuse_state_volume_missing_sftr(
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
            rec("R1", Some(Decimal::from(1000)), None),
            rec("R2", None, None),
            rec("R3", None, None),
        ];
        let (ind, ev) = compute_dqi_reuse_state_volume_missing_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 3);
        assert_eq!(ind.numerator, 2);
        assert_eq!(ev.len(), 2);
        assert_eq!(ev[0].uti, "R2");
        assert_eq!(ev[1].uti, "R3");
    }

    #[test]
    fn err_state_records_not_excluded_unlike_auth_071_version() {
        // Honest divergence from the auth.071 version: auth.086
        // has no Err wrapper concept (it's state, not events),
        // so action_type=REUU or anything else doesn't exempt
        // a record from the denominator. The auth.071 sister
        // computer DOES exempt ERRT records.
        let r = SftrReuseStateRecord {
            record_id: Some("R-NONE-ACTION".into()),
            action_type: None,
            ..Default::default()
        };
        let (ind, _) = compute_dqi_reuse_state_volume_missing_sftr(
            std::slice::from_ref(&r),
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 1);
        assert_eq!(ind.numerator, 1);
    }
}
