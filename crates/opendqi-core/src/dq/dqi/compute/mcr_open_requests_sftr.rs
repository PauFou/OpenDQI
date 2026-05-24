//! `DQI_MCR_OPEN_REQUESTS_SFTR` — share of SFTR Missing
//! Collateral Requests (`auth.083`) for which the requested
//! UTI is still **absent** from the most recent SFTR TSR
//! snapshot (or **all** MCR requests if no TSR companion is
//! provided).
//!
//! - **Layer:** auth.083 SFTR Missing Collateral Request →
//!   [`crate::MissingCollateralRecord`], optionally
//!   cross-referenced against [`crate::SftrTrStateRecord`]
//!   (the auth.079 companion).
//! - **Denominator:** all `MissingCollateralRecord`s
//!   (filtered defensively by `regime == Sftr`).
//! - **Numerator:**
//!   - If a TSR companion is provided: MCR records whose
//!     `uti` is **not** present in the TSR snapshot.
//!   - If no TSR companion: **all** MCR records (the rate
//!     degenerates to 100 % red — every open request is
//!     unresolved from the engine's perspective). The
//!     description string surfaces this degraded mode to
//!     downstream consumers.
//! - **Dimension:** completeness.
//!
//! Mirror of the operational pattern behind the granular
//! `SFTR.MCR.REQUESTED_UTI_NOT_IN_TSR` check : the DQI rolls
//! the per-record check into a single rate suitable for
//! committee reporting.

use std::collections::HashSet;

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, MissingCollateralRecord, Regime, SftrTrStateRecord};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_MCR_OPEN_REQUESTS_SFTR";
const DESCRIPTION_WITH_TSR: &str = "Share of SFTR Missing Collateral Requests (auth.083) \
whose UTI is not present in the SFTR TSR (auth.079) snapshot. Counts only records with \
a UTI populated.";
const DESCRIPTION_NO_TSR: &str = "All SFTR Missing Collateral Requests (auth.083) — no TSR \
(auth.079) companion provided, so every request is treated as unresolved (degraded mode). \
Provide --tsr alongside --missing-collateral for a meaningful rate.";

/// Compute `DQI_MCR_OPEN_REQUESTS_SFTR`.
pub fn compute_dqi_mcr_open_requests_sftr(
    mcr: &[MissingCollateralRecord],
    tsr: Option<&[SftrTrStateRecord]>,
    thresholds: &Thresholds,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    // Pre-index the TSR UTIs once (if provided) for O(1)
    // lookup per MCR record.
    let tsr_utis: Option<HashSet<&str>> = tsr.map(|slice| {
        slice
            .iter()
            .filter(|r| r.regime == Regime::Sftr)
            .filter_map(|r| r.uti.as_deref())
            .collect()
    });

    let mut denominator: u64 = 0;
    let mut numerator: u64 = 0;
    let mut offenders: Vec<DqiEvidence> = Vec::new();

    for r in mcr {
        if r.regime != Regime::Sftr {
            continue;
        }
        // MCR records without a UTI cannot be cross-referenced
        // against the TSR ; exclude from the denominator (the
        // granular SFTR.MCR.REQUEST_WITHOUT_UTI check covers
        // these at row level).
        let Some(uti) = r.uti.as_deref() else {
            continue;
        };
        denominator += 1;
        let unresolved = match tsr_utis.as_ref() {
            Some(set) => !set.contains(uti),
            None => true, // degraded mode: no TSR → all open
        };
        if !unresolved {
            continue;
        }
        numerator += 1;
        offenders.push(DqiEvidence {
            indicator_id: INDICATOR_ID.into(),
            uti: uti.to_owned(),
            counterparty: r.reporting_counterparty.clone(),
            asset_class: None,
            source_file: r.source_file.clone(),
            observed_value: r.master_agreement_type.clone(),
            explanation: match tsr_utis.as_ref() {
                Some(_) => "requested UTI not present in SFTR TSR snapshot".into(),
                None => "no TSR companion provided ; MCR request treated as unresolved".into(),
            },
        });
    }

    offenders.sort_by(|a, b| {
        a.source_file
            .cmp(&b.source_file)
            .then_with(|| a.counterparty.cmp(&b.counterparty))
            .then_with(|| a.uti.cmp(&b.uti))
    });
    offenders.truncate(EVIDENCE_TOP_N);

    let pair = resolve_threshold(thresholds, INDICATOR_ID);
    let (rate, status) = rate_with_status(numerator, denominator, &pair);
    let description = if tsr_utis.is_some() {
        DESCRIPTION_WITH_TSR
    } else {
        DESCRIPTION_NO_TSR
    };
    let indicator = DqiIndicator {
        indicator_id: INDICATOR_ID.into(),
        regime: Regime::Sftr,
        dimension: DqDimension::Completeness,
        table_scope: "auth.083".into(),
        numerator,
        denominator,
        rate,
        threshold_amber: Some(pair.amber),
        threshold_red: Some(pair.red),
        status,
        description: description.into(),
    };
    (indicator, offenders)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dq::dqi::DqiStatus;

    fn mcr(uti: Option<&str>, regime: Regime) -> MissingCollateralRecord {
        MissingCollateralRecord {
            uti: uti.map(|s| s.into()),
            regime,
            ..Default::default()
        }
    }

    fn tsr(uti: &str) -> SftrTrStateRecord {
        SftrTrStateRecord {
            uti: Some(uti.into()),
            regime: Regime::Sftr,
            ..Default::default()
        }
    }

    #[test]
    fn empty_input_is_not_applicable() {
        let (ind, _) = compute_dqi_mcr_open_requests_sftr(
            &[],
            None,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn emir_records_ignored_by_regime_filter() {
        let recs = vec![mcr(Some("E1"), Regime::Emir)];
        let (ind, _) = compute_dqi_mcr_open_requests_sftr(
            &recs,
            None,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 0);
    }

    #[test]
    fn no_uti_records_excluded_from_denominator() {
        let recs = vec![mcr(None, Regime::Sftr)];
        let (ind, _) = compute_dqi_mcr_open_requests_sftr(
            &recs,
            None,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 0);
    }

    #[test]
    fn no_tsr_companion_yields_all_red() {
        // Degraded mode : every MCR with a UTI counts as
        // unresolved.
        let recs = vec![mcr(Some("U1"), Regime::Sftr), mcr(Some("U2"), Regime::Sftr)];
        let (ind, ev) = compute_dqi_mcr_open_requests_sftr(
            &recs,
            None,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 2);
        assert_eq!(ind.numerator, 2);
        assert_eq!(ind.status, DqiStatus::Red);
        assert_eq!(ev.len(), 2);
        assert!(ind.description.contains("no TSR (auth.079) companion"));
    }

    #[test]
    fn tsr_cross_ref_resolves_open_requests() {
        let recs = vec![
            mcr(Some("U1"), Regime::Sftr),
            mcr(Some("U2"), Regime::Sftr),
            mcr(Some("U3"), Regime::Sftr),
        ];
        let tsr_recs = vec![tsr("U1"), tsr("U3")];
        let (ind, ev) = compute_dqi_mcr_open_requests_sftr(
            &recs,
            Some(&tsr_recs),
            &Thresholds::default(),
            MappingPresence::default(),
        );
        // U1 + U3 are in TSR → resolved. U2 still open.
        assert_eq!(ind.denominator, 3);
        assert_eq!(ind.numerator, 1);
        assert_eq!(ev.len(), 1);
        assert_eq!(ev[0].uti, "U2");
        assert!(ind.description.contains("not present in"));
    }

    #[test]
    fn fully_resolved_is_green() {
        let recs = vec![mcr(Some("U1"), Regime::Sftr)];
        let tsr_recs = vec![tsr("U1")];
        let (ind, _) = compute_dqi_mcr_open_requests_sftr(
            &recs,
            Some(&tsr_recs),
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.status, DqiStatus::Green);
    }
}
