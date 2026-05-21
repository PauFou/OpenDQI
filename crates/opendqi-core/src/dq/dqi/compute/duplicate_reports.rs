//! `DQI_DUPLICATE_REPORTS` — share of TSR records whose UTI
//! appears in more than one row in the snapshot.
//!
//! Indicator-level rollup of `EMIR.UNI.DUPLICATE_UTI` +
//! `EMIR.TST.DUPLICATE_ACTIVE_UTI` granular checks. Lets the
//! committee see "what percentage of my outstanding state is
//! duplicate" in one number.
//!
//! - **Layer:** TSR.
//! - **Denominator:** TSR records with a non-empty UTI.
//! - **Numerator:** records whose UTI appears ≥ 2 times in the
//!   snapshot.
//! - **Dimension:** uniqueness.

use std::collections::HashMap;

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, Regime, TrStateRecord};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_DUPLICATE_REPORTS";
const DESCRIPTION: &str = "Share of TSR records whose UTI appears in more than one row in the \
snapshot. Indicator-level rollup of EMIR.UNI.DUPLICATE_UTI + EMIR.TST.DUPLICATE_ACTIVE_UTI \
granular checks.";

/// Compute `DQI_DUPLICATE_REPORTS`.
pub fn compute_dqi_duplicate_reports(
    tsr: &[TrStateRecord],
    thresholds: &Thresholds,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    // First pass: count occurrences of each UTI.
    let mut counts: HashMap<&str, u64> = HashMap::new();
    for r in tsr {
        if let Some(u) = r.uti.as_deref() {
            let u = u.trim();
            if !u.is_empty() {
                *counts.entry(u).or_insert(0) += 1;
            }
        }
    }

    let mut denominator: u64 = 0;
    let mut numerator: u64 = 0;
    let mut offenders: Vec<DqiEvidence> = Vec::new();

    // Second pass: classify each record.
    for r in tsr {
        let Some(uti) = r.uti.as_deref().map(str::trim).filter(|s| !s.is_empty()) else {
            continue;
        };
        denominator += 1;
        let count = counts.get(uti).copied().unwrap_or(0);
        if count < 2 {
            continue;
        }
        numerator += 1;
        offenders.push(DqiEvidence {
            indicator_id: INDICATOR_ID.into(),
            uti: uti.to_string(),
            counterparty: r.reporting_counterparty.clone(),
            asset_class: None,
            source_file: r.source_file.clone(),
            observed_value: Some(count.to_string()),
            explanation: format!("UTI appears {count} times in the TSR snapshot"),
        });
    }

    // Sort by count descending (worst-duplicated first), then UTI alphabetic for ties.
    offenders.sort_by(|a, b| {
        let av = a
            .observed_value
            .as_deref()
            .unwrap_or("0")
            .parse::<u64>()
            .unwrap_or(0);
        let bv = b
            .observed_value
            .as_deref()
            .unwrap_or("0")
            .parse::<u64>()
            .unwrap_or(0);
        bv.cmp(&av).then_with(|| a.uti.cmp(&b.uti))
    });
    offenders.truncate(EVIDENCE_TOP_N);

    let pair = resolve_threshold(thresholds, INDICATOR_ID);
    let (rate, status) = rate_with_status(numerator, denominator, &pair);
    let indicator = DqiIndicator {
        indicator_id: INDICATOR_ID.into(),
        regime: Regime::Emir,
        dimension: DqDimension::Uniqueness,
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

    fn rec(uti: &str) -> TrStateRecord {
        TrStateRecord {
            uti: Some(uti.into()),
            ..Default::default()
        }
    }

    #[test]
    fn empty_is_not_applicable() {
        let (ind, _) =
            compute_dqi_duplicate_reports(&[], &Thresholds::default(), MappingPresence::default());
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn no_duplicates_is_green() {
        let recs = vec![rec("U1"), rec("U2"), rec("U3")];
        let (ind, _) = compute_dqi_duplicate_reports(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn duplicates_count_all_occurrences() {
        // U1 appears 3 times → all 3 records count toward numerator.
        let recs = vec![rec("U1"), rec("U1"), rec("U1"), rec("U2")];
        let (ind, ev) = compute_dqi_duplicate_reports(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 4);
        assert_eq!(ind.numerator, 3);
        // Evidence : 3 rows for U1 (sorted worst-first, then UTI),
        // count visible as observed_value.
        assert_eq!(ev.len(), 3);
        assert_eq!(ev[0].uti, "U1");
        assert_eq!(ev[0].observed_value, Some("3".into()));
    }

    #[test]
    fn empty_utis_excluded_from_denominator() {
        let recs = vec![rec(""), rec("")];
        let (ind, _) = compute_dqi_duplicate_reports(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 0);
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }
}
