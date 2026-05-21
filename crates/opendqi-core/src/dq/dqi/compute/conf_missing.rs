//! `DQI_CONF_MISSING` — share of TAR records (modelled as
//! [`crate::model::EmirRecord`]) whose `confirmation_timestamp`
//! is unset.
//!
//! **Gated**: `confirmation_timestamp` is not a typed field on
//! `EmirRecord` (canonical EMIR model keeps it under
//! `raw_fields` when the user maps it). The orchestrator
//! computes [`MappingPresence::has_confirmation_timestamp`] by
//! inspecting the mapping + observing ≥ 1 non-NULL value; if
//! the gate is **off**, the indicator returns
//! [`crate::dq::dqi::DqiStatus::NotApplicable`] with no rate
//! and no evidence.
//!
//! - **Layer:** TAR.
//! - **Denominator:** TAR records (when gated on).
//! - **Numerator:** TAR records where
//!   `raw_fields["confirmation_timestamp"]` is missing or empty.
//! - **Dimension:** completeness.

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, DqiStatus, MappingPresence};
use crate::model::{DqDimension, EmirRecord, Regime};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_CONF_MISSING";
const DESCRIPTION: &str = "TAR records whose confirmation_timestamp is missing. \
Gated: NotApplicable when the field is not mapped or never observed.";

/// Raw-fields key the computer reads. Matches the canonical
/// EMIR mapping convention.
const FIELD: &str = "confirmation_timestamp";

fn missing(r: &EmirRecord) -> bool {
    r.raw_fields
        .get(FIELD)
        .map(|s| s.trim().is_empty())
        .unwrap_or(true)
}

/// Compute `DQI_CONF_MISSING`.
pub fn compute_dqi_conf_missing(
    tar: &[EmirRecord],
    thresholds: &Thresholds,
    mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    let pair = resolve_threshold(thresholds, INDICATOR_ID);

    if !mapping_presence.has_confirmation_timestamp {
        // Gated off — emit NotApplicable with a self-explanatory
        // description.
        let indicator = DqiIndicator {
            indicator_id: INDICATOR_ID.into(),
            regime: Regime::Emir,
            dimension: DqDimension::Completeness,
            table_scope: "TAR".into(),
            numerator: 0,
            denominator: 0,
            rate: None,
            threshold_amber: Some(pair.amber),
            threshold_red: Some(pair.red),
            status: DqiStatus::NotApplicable,
            description:
                "confirmation_timestamp not mapped or never observed — DQI_CONF_MISSING skipped"
                    .into(),
        };
        return (indicator, Vec::new());
    }

    let mut denominator: u64 = 0;
    let mut numerator: u64 = 0;
    let mut offenders: Vec<DqiEvidence> = Vec::new();

    for r in tar {
        denominator += 1;
        if !missing(r) {
            continue;
        }
        numerator += 1;
        offenders.push(DqiEvidence {
            indicator_id: INDICATOR_ID.into(),
            uti: r.uti.clone().unwrap_or_default(),
            counterparty: r.entity_responsible_for_reporting.clone(),
            asset_class: r.asset_class.clone(),
            source_file: r.source_file.clone(),
            observed_value: None,
            explanation: "confirmation_timestamp is missing or empty".into(),
        });
    }

    offenders.sort_by(|a, b| {
        a.source_file
            .cmp(&b.source_file)
            .then_with(|| a.uti.cmp(&b.uti))
    });
    offenders.truncate(EVIDENCE_TOP_N);

    let (rate, status) = rate_with_status(numerator, denominator, &pair);
    let indicator = DqiIndicator {
        indicator_id: INDICATOR_ID.into(),
        regime: Regime::Emir,
        dimension: DqDimension::Completeness,
        table_scope: "TAR".into(),
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
    use std::collections::BTreeMap;

    use super::*;

    fn rec_with_conf(uti: &str, conf_ts: Option<&str>) -> EmirRecord {
        let mut raw = BTreeMap::new();
        if let Some(ts) = conf_ts {
            raw.insert(FIELD.to_string(), ts.to_string());
        }
        EmirRecord {
            uti: Some(uti.into()),
            raw_fields: raw,
            ..Default::default()
        }
    }

    #[test]
    fn gate_off_returns_not_applicable_with_no_evidence() {
        let recs = vec![rec_with_conf("U1", None)];
        let (ind, ev) = compute_dqi_conf_missing(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(), // gate off
        );
        assert_eq!(ind.status, DqiStatus::NotApplicable);
        assert_eq!(ind.rate, None);
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.denominator, 0);
        assert!(ev.is_empty());
    }

    #[test]
    fn all_confirmed_is_green() {
        let recs = vec![
            rec_with_conf("U1", Some("2026-05-20T09:00:00Z")),
            rec_with_conf("U2", Some("2026-05-20T10:00:00Z")),
        ];
        let presence = MappingPresence {
            has_confirmation_timestamp: true,
            ..Default::default()
        };
        let (ind, _) = compute_dqi_conf_missing(&recs, &Thresholds::default(), presence);
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.denominator, 2);
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn missing_counts_and_emits_evidence() {
        let recs = vec![
            rec_with_conf("U1", None),
            rec_with_conf("U2", Some("2026-05-20T09:00:00Z")),
            rec_with_conf("U3", Some("")), // empty string counts as missing
        ];
        let presence = MappingPresence {
            has_confirmation_timestamp: true,
            ..Default::default()
        };
        let (ind, ev) = compute_dqi_conf_missing(&recs, &Thresholds::default(), presence);
        assert_eq!(ind.denominator, 3);
        assert_eq!(ind.numerator, 2);
        assert_eq!(ev.len(), 2);
    }
}
