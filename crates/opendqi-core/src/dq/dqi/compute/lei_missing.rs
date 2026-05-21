//! `DQI_LEI_MISSING` — share of TSR records where the
//! reporting counterparty or other counterparty LEI is
//! missing or empty.
//!
//! - **Layer:** TSR.
//! - **Denominator:** all TSR records.
//! - **Numerator:** records where `reporting_counterparty` OR
//!   `other_counterparty` is `None` or whitespace-only.
//! - **Dimension:** completeness.
//!
//! v0.16 ships a presence-only check. LEI **format**
//! validation (ISO 17442 check digit) remains the
//! granular `EMIR.VLD.LEI_FORMAT_{RC,OC,CCP,ERR}` family.
//! This DQI is the rollup view ; the per-row format issues
//! still appear in `issues.csv`.

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, Regime, TrStateRecord};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_LEI_MISSING";
const DESCRIPTION: &str = "Share of TSR records with at least one missing/empty counterparty LEI \
(reporting or other). Format validation (ISO 17442 check digit) remains in the EMIR.VLD.LEI_FORMAT_* \
granular checks.";

fn lei_missing(s: Option<&str>) -> bool {
    s.map(|v| v.trim().is_empty()).unwrap_or(true)
}

/// Compute `DQI_LEI_MISSING`.
pub fn compute_dqi_lei_missing(
    tsr: &[TrStateRecord],
    thresholds: &Thresholds,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    let mut denominator: u64 = 0;
    let mut numerator: u64 = 0;
    let mut offenders: Vec<DqiEvidence> = Vec::new();

    for r in tsr {
        denominator += 1;
        let rc_missing = lei_missing(r.reporting_counterparty.as_deref());
        let oc_missing = lei_missing(r.other_counterparty.as_deref());
        if !rc_missing && !oc_missing {
            continue;
        }
        numerator += 1;
        let which = match (rc_missing, oc_missing) {
            (true, true) => "both RC + OC LEIs",
            (true, false) => "reporting_counterparty LEI",
            (false, true) => "other_counterparty LEI",
            (false, false) => unreachable!(),
        };
        offenders.push(DqiEvidence {
            indicator_id: INDICATOR_ID.into(),
            uti: r.uti.clone().unwrap_or_default(),
            counterparty: r.reporting_counterparty.clone(),
            asset_class: None,
            source_file: r.source_file.clone(),
            observed_value: None,
            explanation: format!("missing {which}"),
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
        table_scope: "TSR".into(),
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

    fn rec(uti: &str, rc: Option<&str>, oc: Option<&str>) -> TrStateRecord {
        TrStateRecord {
            uti: Some(uti.into()),
            reporting_counterparty: rc.map(|s| s.into()),
            other_counterparty: oc.map(|s| s.into()),
            ..Default::default()
        }
    }

    #[test]
    fn empty_is_not_applicable() {
        let (ind, _) =
            compute_dqi_lei_missing(&[], &Thresholds::default(), MappingPresence::default());
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn both_leis_present_is_green() {
        let recs = vec![
            rec(
                "U1",
                Some("LEI00000000000000001A"),
                Some("LEI00000000000000002B"),
            ),
            rec(
                "U2",
                Some("LEI00000000000000003C"),
                Some("LEI00000000000000004D"),
            ),
        ];
        let (ind, _) =
            compute_dqi_lei_missing(&recs, &Thresholds::default(), MappingPresence::default());
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn missing_rc_lei_counts() {
        let recs = vec![rec("U1", None, Some("LEI00000000000000002B"))];
        let (ind, ev) =
            compute_dqi_lei_missing(&recs, &Thresholds::default(), MappingPresence::default());
        assert_eq!(ind.numerator, 1);
        assert!(ev[0].explanation.contains("reporting_counterparty LEI"));
    }

    #[test]
    fn empty_string_lei_counts_as_missing() {
        let recs = vec![rec("U1", Some(""), Some("LEI00000000000000002B"))];
        let (ind, _) =
            compute_dqi_lei_missing(&recs, &Thresholds::default(), MappingPresence::default());
        assert_eq!(ind.numerator, 1);
    }
}
