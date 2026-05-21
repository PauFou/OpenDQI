//! `DQI_REC_STATUS_UNPAIRED` — share of records whose
//! TR-provided reconciliation status is `unpaired`/equivalent.
//!
//! **Gated**: `reconciliation_status` is not a typed field on
//! `EmirRecord` (the TR sometimes ships it inline on TAR/TSR
//! rows, sometimes in a dedicated reconciliation report). The
//! orchestrator computes
//! [`MappingPresence::has_reconciliation_status`] from the
//! mapping + observation; if the gate is **off**, the indicator
//! returns [`crate::dq::dqi::DqiStatus::NotApplicable`] with no
//! evidence.
//!
//! - **Layer:** records carrying a `reconciliation_status` field
//!   (TAR or TSR; v0.15 wires it on TAR, mirrors on TSR in
//!   v0.16).
//! - **Denominator:** records where `reconciliation_status` is
//!   set.
//! - **Numerator:** records whose lowercased value matches
//!   `unpaired` / `not_paired` / `unrec` / `unreconciled` /
//!   `unmatched`.
//! - **Dimension:** consistency.

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, DqiStatus, MappingPresence};
use crate::model::{DqDimension, EmirRecord, Regime};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_REC_STATUS_UNPAIRED";
const DESCRIPTION: &str = "Records whose TR-provided reconciliation_status indicates unpaired \
or unreconciled. Gated: NotApplicable when the field is not mapped or never observed.";

const FIELD: &str = "reconciliation_status";

/// Token set that means "the TR did not pair / reconcile this".
/// Lower-cased before comparison.
const UNPAIRED_TOKENS: &[&str] = &[
    "unpaired",
    "not_paired",
    "unrec",
    "unreconciled",
    "unmatched",
];

fn status_token(r: &EmirRecord) -> Option<String> {
    r.raw_fields
        .get(FIELD)
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
}

/// Compute `DQI_REC_STATUS_UNPAIRED`.
pub fn compute_dqi_rec_status_unpaired(
    records: &[EmirRecord],
    thresholds: &Thresholds,
    mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    let pair = resolve_threshold(thresholds, INDICATOR_ID);

    if !mapping_presence.has_reconciliation_status {
        let indicator = DqiIndicator {
            indicator_id: INDICATOR_ID.into(),
            regime: Regime::Emir,
            dimension: DqDimension::Consistency,
            table_scope: "TAR".into(),
            numerator: 0,
            denominator: 0,
            rate: None,
            threshold_amber: Some(pair.amber),
            threshold_red: Some(pair.red),
            status: DqiStatus::NotApplicable,
            description: "reconciliation_status not mapped or never observed — \
DQI_REC_STATUS_UNPAIRED skipped"
                .into(),
        };
        return (indicator, Vec::new());
    }

    let mut denominator: u64 = 0;
    let mut numerator: u64 = 0;
    let mut offenders: Vec<DqiEvidence> = Vec::new();

    for r in records {
        let Some(tok) = status_token(r) else {
            continue;
        };
        denominator += 1;
        if !UNPAIRED_TOKENS.contains(&tok.as_str()) {
            continue;
        }
        numerator += 1;
        offenders.push(DqiEvidence {
            indicator_id: INDICATOR_ID.into(),
            uti: r.uti.clone().unwrap_or_default(),
            counterparty: r.entity_responsible_for_reporting.clone(),
            asset_class: r.asset_class.clone(),
            source_file: r.source_file.clone(),
            observed_value: Some(tok),
            explanation: "TR-provided reconciliation_status is unpaired / unreconciled".into(),
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
        dimension: DqDimension::Consistency,
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

    fn rec_with_status(uti: &str, status: Option<&str>) -> EmirRecord {
        let mut raw = BTreeMap::new();
        if let Some(s) = status {
            raw.insert(FIELD.to_string(), s.to_string());
        }
        EmirRecord {
            uti: Some(uti.into()),
            raw_fields: raw,
            ..Default::default()
        }
    }

    #[test]
    fn gate_off_returns_not_applicable_with_no_evidence() {
        let recs = vec![rec_with_status("U1", Some("unpaired"))];
        let (ind, ev) = compute_dqi_rec_status_unpaired(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.status, DqiStatus::NotApplicable);
        assert_eq!(ind.rate, None);
        assert!(ev.is_empty());
    }

    #[test]
    fn unmapped_records_excluded_from_denominator_even_when_gate_on() {
        let recs = vec![rec_with_status("U1", None), rec_with_status("U2", None)];
        let presence = MappingPresence {
            has_reconciliation_status: true,
            ..Default::default()
        };
        let (ind, _) = compute_dqi_rec_status_unpaired(&recs, &Thresholds::default(), presence);
        assert_eq!(ind.denominator, 0);
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn all_paired_is_green() {
        let recs = vec![
            rec_with_status("U1", Some("paired")),
            rec_with_status("U2", Some("PAIRED")),
            rec_with_status("U3", Some("reconciled")),
        ];
        let presence = MappingPresence {
            has_reconciliation_status: true,
            ..Default::default()
        };
        let (ind, _) = compute_dqi_rec_status_unpaired(&recs, &Thresholds::default(), presence);
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.denominator, 3);
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn unpaired_tokens_breach() {
        let recs = vec![
            rec_with_status("U1", Some("UNPAIRED")),
            rec_with_status("U2", Some("not_paired")),
            rec_with_status("U3", Some("unreconciled")),
            rec_with_status("U4", Some("paired")),
        ];
        let presence = MappingPresence {
            has_reconciliation_status: true,
            ..Default::default()
        };
        let (ind, ev) = compute_dqi_rec_status_unpaired(&recs, &Thresholds::default(), presence);
        assert_eq!(ind.denominator, 4);
        assert_eq!(ind.numerator, 3);
        assert_eq!(ev.len(), 3);
    }
}
