//! `DQI_ERR_MISSING` — share of EmirRecord (TAR / submission)
//! records where `entity_responsible_for_reporting` is missing
//! or empty.
//!
//! - **Layer:** TAR.
//! - **Denominator:** all EmirRecord rows passed in.
//! - **Numerator:** records with no ERR LEI.
//! - **Dimension:** completeness.

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, EmirRecord, Regime};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_ERR_MISSING";
const DESCRIPTION: &str =
    "Share of TAR records with no entity_responsible_for_reporting (ERR) LEI.";

/// Compute `DQI_ERR_MISSING`.
pub fn compute_dqi_err_missing(
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
            .entity_responsible_for_reporting
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
            counterparty: r.counterparty_1.clone(),
            asset_class: r.asset_class.clone(),
            source_file: r.source_file.clone(),
            observed_value: None,
            explanation: "entity_responsible_for_reporting LEI missing or empty".into(),
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

    fn rec(uti: &str, err: Option<&str>) -> EmirRecord {
        EmirRecord {
            uti: Some(uti.into()),
            entity_responsible_for_reporting: err.map(|s| s.into()),
            ..Default::default()
        }
    }

    #[test]
    fn empty_is_not_applicable() {
        let (ind, _) =
            compute_dqi_err_missing(&[], &Thresholds::default(), MappingPresence::default());
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn err_present_is_green() {
        let recs = vec![rec("U1", Some("LEI00000000000000001A"))];
        let (ind, _) =
            compute_dqi_err_missing(&recs, &Thresholds::default(), MappingPresence::default());
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn missing_err_counts() {
        let recs = vec![rec("U1", None), rec("U2", Some(""))];
        let (ind, _) =
            compute_dqi_err_missing(&recs, &Thresholds::default(), MappingPresence::default());
        assert_eq!(ind.numerator, 2);
    }
}
