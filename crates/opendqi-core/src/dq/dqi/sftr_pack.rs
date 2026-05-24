//! SFTR Data Quality Pack orchestrator.
//!
//! Mirror of the EMIR pack architecture
//! ([`crate::dq::dqi::compute_emir_dqi_pack`]) for SFTR
//! inputs.
//!
//! v0.16 shipped **4 T2-layer SFTR DQIs** drawn from
//! `auth.079` + `auth.052` :
//!
//! - `DQI_LOAN_VALUE_MISSING_SFTR`
//! - `DQI_LOAN_VALUE_STALE_SFTR`
//! - `DQI_COLLATERAL_VALUE_MISSING_SFTR`
//! - `DQI_TIM_REPORTING_LATE_SFTR`
//!
//! v0.17 expands to **16 SFTR DQIs** by adding the T3 margin
//! layer (4 DQIs sourced from the new `msr` input slot,
//! parsed from `auth.085`), the reconciliation layer (4 DQIs
//! sourced from the now-consumed `reconciliation` slot,
//! parsed from `auth.080`), the missing-collateral layer (1
//! DQI sourced from the now-consumed `missing_collateral`
//! slot, parsed from `auth.083`), and 3 SFTR-specific TSR
//! DQIs (haircut anomaly, LEI missing, under-
//! collateralization).
//!
//! The MSR (margin state) layer landed at A4 of v0.17 ; the
//! 4 T3 computers that read it are added in B1'. Until then
//! the `msr` slot is plumbed through the struct but unused
//! by any computer — providing it today is parsed-and-stored,
//! ready for B1'.

use chrono::{DateTime, NaiveDate, Utc};

use crate::dq::aggregate::IssueAggregator;
use crate::dq::dqi::compute::{
    compute_dqi_collateral_value_missing_sftr, compute_dqi_loan_value_missing_sftr,
    compute_dqi_loan_value_stale_sftr, compute_dqi_tim_reporting_late_sftr,
};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, DqiPackResult, DqiStatus, MappingPresence};
use crate::dq::{
    default_sftr_checks, default_sftr_tr_state_checks, run_all_sftr, run_all_sftr_tr_state,
    CheckContext,
};
use crate::model::{
    DqDimension, DqIssue, Regime, SftrMarginStateRecord, SftrRecord, SftrTrStateRecord,
};
use crate::Thresholds;

/// All SFTR DQI inputs, each optional.
///
/// Layer mapping :
/// - `tsr` + `tar` are read by 4 v0.16 T2-layer DQIs (loan
///   missing/stale, collateral missing, reporting timeliness)
///   and remain unchanged in v0.17.
/// - `reconciliation` powers the 4 v0.17 SFTR reconciliation
///   DQIs (`DQI_PAIRING_RATE_SFTR`,
///   `DQI_RECONCILIATION_RATE_SFTR`,
///   `DQI_UNPAIRED_TRADES_RATE_SFTR`,
///   `DQI_FIELD_MISMATCH_RATE_SFTR` — added in C1).
/// - `missing_collateral` powers the 1 v0.17 MCR rollup DQI
///   `DQI_MCR_OPEN_REQUESTS_SFTR` (added in D1).
/// - `msr` powers the 4 v0.17 T3 margin DQIs sourced from
///   `auth.085` (added in B1').
///
/// **A4 status**: the `msr` field is plumbed but no computer
/// reads it yet — wiring lands in B1'. The same was true of
/// `reconciliation` and `missing_collateral` in v0.16 ; both
/// become live in v0.17 C1/D1.
#[derive(Debug, Default, Clone)]
pub struct SftrDqiInputs<'a> {
    /// SFTR Trade State Report (`auth.079`) — projected onto
    /// [`SftrTrStateRecord`].
    pub tsr: Option<&'a [SftrTrStateRecord]>,
    /// SFTR Trade Activity Report (`auth.052`) — projected
    /// onto [`SftrRecord`].
    pub tar: Option<&'a [SftrRecord]>,
    /// SFTR Reconciliation Status Advice (`auth.080`) —
    /// powers the 4 SFTR reconciliation DQIs added in C1.
    pub reconciliation: Option<&'a [crate::ReconciliationRecord]>,
    /// SFTR Missing-Collateral request (`auth.083`) — powers
    /// the MCR rollup DQI added in D1.
    pub missing_collateral: Option<&'a [crate::MissingCollateralRecord]>,
    /// SFTR Margin Data Transaction State Report (`auth.085`)
    /// — projected onto [`SftrMarginStateRecord`]. Portfolio-
    /// level margin state (CCP-cleared SFTs only). Powers the
    /// 4 SFTR T3 margin DQIs added in B1'.
    pub msr: Option<&'a [SftrMarginStateRecord]>,
}

/// Run the SFTR Data Quality Pack.
///
/// Returns a [`DqiPackResult`] with 4 SFTR indicators in
/// `indicator_id` order, evidence rows, the granular issues
/// from running the existing SFTR check registries on each
/// provided layer, and a `ScanSummary` over those issues.
///
/// **No I/O, no parsing** — pure function. The CLI / Python
/// layer parses the input files and passes slices in.
pub fn compute_sftr_dqi_pack(
    inputs: SftrDqiInputs<'_>,
    mapping_presence: MappingPresence,
    thresholds: &Thresholds,
    as_of: NaiveDate,
) -> DqiPackResult {
    let started_at = Utc::now();

    // ---------- DQI layer (4 indicators in v0.16) ----------
    let mut indicators: Vec<DqiIndicator> = Vec::with_capacity(4);
    let mut evidence: Vec<DqiEvidence> = Vec::new();

    let mut push = |ind: DqiIndicator, mut ev: Vec<DqiEvidence>| {
        indicators.push(ind);
        evidence.append(&mut ev);
    };

    // TSR-only indicators (3): LOAN_VALUE_MISSING_SFTR,
    // LOAN_VALUE_STALE_SFTR, COLLATERAL_VALUE_MISSING_SFTR.
    if let Some(tsr) = inputs.tsr {
        let (ind, ev) = compute_dqi_loan_value_missing_sftr(tsr, thresholds, mapping_presence);
        push(ind, ev);
        let (ind, ev) = compute_dqi_loan_value_stale_sftr(tsr, thresholds, as_of, mapping_presence);
        push(ind, ev);
        let (ind, ev) =
            compute_dqi_collateral_value_missing_sftr(tsr, thresholds, mapping_presence);
        push(ind, ev);
    } else {
        push(
            not_applicable("DQI_LOAN_VALUE_MISSING_SFTR", "SFTR TSR not provided"),
            Vec::new(),
        );
        push(
            not_applicable("DQI_LOAN_VALUE_STALE_SFTR", "SFTR TSR not provided"),
            Vec::new(),
        );
        push(
            not_applicable("DQI_COLLATERAL_VALUE_MISSING_SFTR", "SFTR TSR not provided"),
            Vec::new(),
        );
    }

    // TAR-only indicator (1): TIM_REPORTING_LATE_SFTR.
    if let Some(tar) = inputs.tar {
        let (ind, ev) = compute_dqi_tim_reporting_late_sftr(tar, thresholds, mapping_presence);
        push(ind, ev);
    } else {
        push(
            not_applicable("DQI_TIM_REPORTING_LATE_SFTR", "SFTR TAR not provided"),
            Vec::new(),
        );
    }

    // Stable order: sort by indicator_id ascending.
    indicators.sort_by(|a, b| a.indicator_id.cmp(&b.indicator_id));

    // ---------- granular issues layer ----------
    let ctx = build_ctx(thresholds.clone(), as_of);
    let mut all_issues: Vec<DqIssue> = Vec::new();
    let mut files_processed: u32 = 0;
    let mut records_processed: u32 = 0;

    if let Some(tar) = inputs.tar {
        files_processed += 1;
        records_processed = records_processed.saturating_add(tar.len() as u32);
        all_issues.append(&mut run_all_sftr(&default_sftr_checks(), tar, &ctx));
    }
    if let Some(tsr) = inputs.tsr {
        files_processed += 1;
        records_processed = records_processed.saturating_add(tsr.len() as u32);
        let prior: &[SftrRecord] = &[];
        all_issues.append(&mut run_all_sftr_tr_state(
            &default_sftr_tr_state_checks(),
            tsr,
            prior,
            &ctx,
        ));
    }

    let aggregator = IssueAggregator::from_issues(&all_issues);
    let finished_at = Utc::now();
    let issues_summary = aggregator.into_summary(
        Regime::Sftr,
        files_processed,
        records_processed,
        started_at,
        finished_at,
    );

    DqiPackResult {
        indicators,
        evidence,
        issues_summary,
        issues: all_issues,
    }
}

fn not_applicable(indicator_id: &str, reason: &str) -> DqiIndicator {
    DqiIndicator {
        indicator_id: indicator_id.into(),
        regime: Regime::Sftr,
        dimension: DqDimension::Completeness,
        table_scope: "n/a".into(),
        numerator: 0,
        denominator: 0,
        rate: None,
        threshold_amber: None,
        threshold_red: None,
        status: DqiStatus::NotApplicable,
        description: reason.into(),
    }
}

fn build_ctx(thresholds: Thresholds, as_of: NaiveDate) -> CheckContext {
    let now: DateTime<Utc> = as_of
        .and_hms_opt(0, 0, 0)
        .unwrap_or_else(|| {
            NaiveDate::from_ymd_opt(1970, 1, 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap()
        })
        .and_utc();
    CheckContext {
        thresholds,
        today: as_of,
        now,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn as_of() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 5, 21).unwrap()
    }

    #[test]
    fn empty_inputs_yield_4_not_applicable_indicators() {
        let result = compute_sftr_dqi_pack(
            SftrDqiInputs::default(),
            MappingPresence::default(),
            &Thresholds::default(),
            as_of(),
        );
        assert_eq!(
            result.indicators.len(),
            4,
            "always exactly 4 SFTR indicators in v0.16"
        );
        for ind in &result.indicators {
            assert_eq!(
                ind.status,
                DqiStatus::NotApplicable,
                "{} should be NotApplicable on empty input",
                ind.indicator_id
            );
            assert_eq!(ind.regime, Regime::Sftr);
        }
    }

    #[test]
    fn default_inputs_have_msr_slot_at_none() {
        // v0.17 A4 contract: the new msr slot exists and
        // defaults to None alongside the v0.16 slots. Locks
        // the public struct shape so B1' (the 4 T3 computers
        // that read msr) can land without further struct churn.
        let inputs: SftrDqiInputs = SftrDqiInputs::default();
        assert!(inputs.tsr.is_none());
        assert!(inputs.tar.is_none());
        assert!(inputs.reconciliation.is_none());
        assert!(inputs.missing_collateral.is_none());
        assert!(inputs.msr.is_none());
    }

    #[test]
    fn providing_msr_without_computers_is_a_noop_in_v0_17_a4() {
        // Until B1' lands, providing an `msr` slice has no
        // observable effect: indicator count stays at 4 (the
        // T2-layer v0.16 set), every DQI is NotApplicable on
        // empty TSR/TAR, no granular issue is produced. This
        // mirrors how the `reconciliation` / `missing_collateral`
        // slots behaved in v0.16 before C1/D1 wired them.
        let msr: Vec<SftrMarginStateRecord> = vec![];
        let inputs = SftrDqiInputs {
            msr: Some(&msr),
            ..SftrDqiInputs::default()
        };
        let result = compute_sftr_dqi_pack(
            inputs,
            MappingPresence::default(),
            &Thresholds::default(),
            as_of(),
        );
        assert_eq!(result.indicators.len(), 4);
        assert_eq!(result.issues.len(), 0);
    }

    #[test]
    fn indicators_alphabetical_sftr() {
        let result = compute_sftr_dqi_pack(
            SftrDqiInputs::default(),
            MappingPresence::default(),
            &Thresholds::default(),
            as_of(),
        );
        let ids: Vec<&str> = result
            .indicators
            .iter()
            .map(|i| i.indicator_id.as_str())
            .collect();
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted);
        assert_eq!(
            ids,
            vec![
                "DQI_COLLATERAL_VALUE_MISSING_SFTR",
                "DQI_LOAN_VALUE_MISSING_SFTR",
                "DQI_LOAN_VALUE_STALE_SFTR",
                "DQI_TIM_REPORTING_LATE_SFTR",
            ]
        );
    }
}
