//! `DQI_NATURE_MISSING` — share of EmirRecord (TAR) records
//! where the counterparty `nature` (FC / NFC / NFC+) is
//! missing or empty.
//!
//! Required by Article 11 risk-mitigation rules — without
//! this classifier the firm cannot apply the right
//! clearing thresholds, IM cadence, or confirmation deadlines.
//!
//! - **Layer:** TAR.
//! - **Denominator:** all EmirRecord rows.
//! - **Numerator:** records with no `nature` value.
//! - **Dimension:** completeness.

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, EmirRecord, Regime};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_NATURE_MISSING";
const DESCRIPTION: &str = "Share of TAR records with no counterparty nature (FC/NFC/NFC+) classifier. \
Without it the firm cannot apply the right Art.11 clearing threshold / IM cadence / confirmation deadlines.";

/// Compute `DQI_NATURE_MISSING`.
pub fn compute_dqi_nature_missing(
    tar: &[EmirRecord],
    thresholds: &Thresholds,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    let mut denominator: u64 = 0;
    let mut numerator: u64 = 0;
    let mut offenders: Vec<DqiEvidence> = Vec::new();

    for r in tar {
        denominator += 1;
        let missing = r
            .nature
            .as_deref()
            .map(|s| s.trim().is_empty())
            .unwrap_or(true);
        if !missing {
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
            explanation: "counterparty nature (FC/NFC/NFC+) missing".into(),
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
    use super::*;
    use crate::dq::dqi::DqiStatus;

    fn rec(uti: &str, nature: Option<&str>) -> EmirRecord {
        EmirRecord {
            uti: Some(uti.into()),
            nature: nature.map(|s| s.into()),
            ..Default::default()
        }
    }

    #[test]
    fn empty_is_not_applicable() {
        let (ind, _) =
            compute_dqi_nature_missing(&[], &Thresholds::default(), MappingPresence::default());
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn all_present_is_green() {
        let recs = vec![rec("U1", Some("FC")), rec("U2", Some("NFC+"))];
        let (ind, _) =
            compute_dqi_nature_missing(&recs, &Thresholds::default(), MappingPresence::default());
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn missing_counts() {
        let recs = vec![rec("U1", None)];
        let (ind, _) =
            compute_dqi_nature_missing(&recs, &Thresholds::default(), MappingPresence::default());
        assert_eq!(ind.numerator, 1);
    }
}
