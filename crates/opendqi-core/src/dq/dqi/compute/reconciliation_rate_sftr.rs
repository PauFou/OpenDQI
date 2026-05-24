//! `DQI_RECONCILIATION_RATE_SFTR` — share of SFTR per-trade
//! reconciliation records that are **paired** with the
//! counterparty but flagged **unreconciled** at the field
//! level.
//!
//! - **Layer:** auth.080 → [`crate::ReconciliationRecord`]
//!   filtered by `regime == Sftr` (defensive).
//! - **Denominator:** SFTR records with
//!   `pairing_status == PAIRED` (case-insensitive). Only
//!   paired trades carry a meaningful reconciliation status.
//! - **Numerator:** SFTR paired records whose
//!   `reconciliation_status` (case-insensitive) is one of
//!   `UNRECONCILED`, `NOT_RECONCILED`, `UNREC`.
//! - **Dimension:** consistency.
//!
//! This DQI is the SFTR mirror of EMIR's auth.091-derived
//! reconciliation-rate indicator but uses per-trade records
//! (the only data shape auth.080 exposes) rather than per-CP
//! cohort statistics.

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, ReconciliationRecord, Regime};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_RECONCILIATION_RATE_SFTR";
const DESCRIPTION: &str = "Share of paired SFTR trades flagged UNRECONCILED at the field level \
(from auth.080 SFTR Reconciliation Status Advice). Denominator restricted to records with \
pairing_status=PAIRED — unpaired trades have no field-level reconciliation status by definition.";

fn is_paired(status: Option<&str>) -> bool {
    status
        .map(|s| s.trim().to_ascii_uppercase())
        .map(|s| s == "PAIRED")
        .unwrap_or(false)
}

fn is_unreconciled(status: Option<&str>) -> bool {
    status
        .map(|s| s.trim().to_ascii_uppercase())
        .map(|s| s.starts_with("UNREC") || s == "NOT_RECONCILED")
        .unwrap_or(false)
}

/// Compute `DQI_RECONCILIATION_RATE_SFTR`.
pub fn compute_dqi_reconciliation_rate_sftr(
    recon_records: &[ReconciliationRecord],
    thresholds: &Thresholds,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    let mut denominator: u64 = 0;
    let mut numerator: u64 = 0;
    let mut offenders: Vec<DqiEvidence> = Vec::new();

    for r in recon_records {
        if r.regime != Regime::Sftr {
            continue;
        }
        if !is_paired(r.pairing_status.as_deref()) {
            continue;
        }
        denominator += 1;
        if !is_unreconciled(r.reconciliation_status.as_deref()) {
            continue;
        }
        numerator += 1;
        offenders.push(DqiEvidence {
            indicator_id: INDICATOR_ID.into(),
            uti: r.uti.clone().unwrap_or_default(),
            counterparty: r.reporting_counterparty.clone(),
            asset_class: None,
            source_file: r.source_file.clone(),
            observed_value: r.reconciliation_status.clone(),
            explanation: "paired SFTR trade flagged UNRECONCILED on at least one field".into(),
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
    let indicator = DqiIndicator {
        indicator_id: INDICATOR_ID.into(),
        regime: Regime::Sftr,
        dimension: DqDimension::Consistency,
        table_scope: "auth.080".into(),
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

    fn rec(
        uti: &str,
        regime: Regime,
        pairing: Option<&str>,
        recon: Option<&str>,
    ) -> ReconciliationRecord {
        ReconciliationRecord {
            uti: Some(uti.into()),
            regime,
            pairing_status: pairing.map(|s| s.into()),
            reconciliation_status: recon.map(|s| s.into()),
            ..Default::default()
        }
    }

    #[test]
    fn empty_input_is_not_applicable() {
        let (ind, _) = compute_dqi_reconciliation_rate_sftr(
            &[],
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn emir_records_ignored_by_regime_filter() {
        let recs = vec![rec(
            "E1",
            Regime::Emir,
            Some("PAIRED"),
            Some("UNRECONCILED"),
        )];
        let (ind, _) = compute_dqi_reconciliation_rate_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 0);
    }

    #[test]
    fn unpaired_excluded_from_denominator() {
        // unreconciled is meaningless without a pair — exclude.
        let recs = vec![rec(
            "S1",
            Regime::Sftr,
            Some("UNPAIRED"),
            Some("UNRECONCILED"),
        )];
        let (ind, _) = compute_dqi_reconciliation_rate_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 0);
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn paired_and_reconciled_is_green() {
        let recs = vec![
            rec("S1", Regime::Sftr, Some("PAIRED"), Some("RECONCILED")),
            rec("S2", Regime::Sftr, Some("PAIRED"), Some("RECONCILED")),
        ];
        let (ind, _) = compute_dqi_reconciliation_rate_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn paired_and_unreconciled_short_codes_count() {
        let recs = vec![
            rec("S1", Regime::Sftr, Some("PAIRED"), Some("UNRECONCILED")),
            rec("S2", Regime::Sftr, Some("PAIRED"), Some("UNREC")),
            rec("S3", Regime::Sftr, Some("PAIRED"), Some("NOT_RECONCILED")),
            rec("S4", Regime::Sftr, Some("PAIRED"), Some("RECONCILED")),
        ];
        let (ind, ev) = compute_dqi_reconciliation_rate_sftr(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 4);
        assert_eq!(ind.numerator, 3);
        assert_eq!(ev.len(), 3);
    }
}
