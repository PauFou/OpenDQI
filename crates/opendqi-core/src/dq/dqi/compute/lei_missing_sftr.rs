//! `DQI_LEI_MISSING_SFTR` — share of SFTR TSR records with at
//! least one counterparty LEI missing (`reporting_counterparty`
//! OR `other_counterparty`).
//!
//! - **Layer:** SFTR TSR (`auth.079`).
//! - **Denominator:** all SFTR TSR records.
//! - **Numerator:** records where `reporting_counterparty` is
//!   None/empty OR `other_counterparty` is None/empty.
//! - **Dimension:** completeness.
//!
//! SFTR mirror of EMIR's [`crate::dq::dqi::compute_dqi_lei_missing`].
//! Both LEIs are mandatory in the SFTR XSD (`Counterparty39__1`
//! requires `RptgCtrPty` and `OthrCtrPty`), so missing values
//! signal either a parsing failure, a non-LEI natural person
//! identifier on the other-CP side, or a corrupt feed.

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, Regime, SftrTrStateRecord};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_LEI_MISSING_SFTR";
const DESCRIPTION: &str = "Share of SFTR TSR records with at least one counterparty LEI missing \
(reporting_counterparty OR other_counterparty None/empty). SFTR mirror of DQI_LEI_MISSING.";

fn is_empty(s: Option<&str>) -> bool {
    s.map(|v| v.trim().is_empty()).unwrap_or(true)
}

/// Compute `DQI_LEI_MISSING_SFTR`.
pub fn compute_dqi_lei_missing_sftr(
    tsr: &[SftrTrStateRecord],
    thresholds: &Thresholds,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    let mut denominator: u64 = 0;
    let mut numerator: u64 = 0;
    let mut offenders: Vec<DqiEvidence> = Vec::new();

    for r in tsr {
        denominator += 1;
        let rc_missing = is_empty(r.reporting_counterparty.as_deref());
        let oc_missing = is_empty(r.other_counterparty.as_deref());
        if !(rc_missing || oc_missing) {
            continue;
        }
        numerator += 1;
        let which = match (rc_missing, oc_missing) {
            (true, true) => "both LEIs missing",
            (true, false) => "reporting_counterparty LEI missing",
            (false, true) => "other_counterparty LEI missing",
            (false, false) => unreachable!(),
        };
        offenders.push(DqiEvidence {
            indicator_id: INDICATOR_ID.into(),
            uti: r.uti.clone().unwrap_or_default(),
            counterparty: r.reporting_counterparty.clone(),
            asset_class: r.sft_type.clone(),
            source_file: r.source_file.clone(),
            observed_value: None,
            explanation: which.into(),
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
        table_scope: "SFTR-TSR".into(),
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

    fn rec(uti: &str, rc: Option<&str>, oc: Option<&str>) -> SftrTrStateRecord {
        SftrTrStateRecord {
            uti: Some(uti.into()),
            reporting_counterparty: rc.map(|s| s.into()),
            other_counterparty: oc.map(|s| s.into()),
            ..Default::default()
        }
    }

    #[test]
    fn empty_input_is_not_applicable() {
        let (ind, _) =
            compute_dqi_lei_missing_sftr(&[], &Thresholds::default(), MappingPresence::default());
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn both_present_is_green() {
        let recs = vec![rec(
            "U1",
            Some("RPTGCPARTY0000000001"),
            Some("OTHRCPARTY0000000002"),
        )];
        let (ind, _) =
            compute_dqi_lei_missing_sftr(&recs, &Thresholds::default(), MappingPresence::default());
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn either_side_missing_fires() {
        let recs = vec![
            rec("U1", None, Some("OTHR")),
            rec("U2", Some("RPTG"), None),
            rec("U3", Some(""), Some("OTHR")), // empty string = missing
            rec("U4", Some("RPTG"), Some("OTHR")),
        ];
        let (ind, ev) =
            compute_dqi_lei_missing_sftr(&recs, &Thresholds::default(), MappingPresence::default());
        assert_eq!(ind.denominator, 4);
        assert_eq!(ind.numerator, 3);
        assert_eq!(ev.len(), 3);
    }

    #[test]
    fn both_missing_records_explanation() {
        let recs = vec![rec("U1", None, None)];
        let (_, ev) =
            compute_dqi_lei_missing_sftr(&recs, &Thresholds::default(), MappingPresence::default());
        assert_eq!(ev.len(), 1);
        assert_eq!(ev[0].explanation, "both LEIs missing");
    }
}
