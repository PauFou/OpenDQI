//! SFTR Data Quality Pack orchestrator (v0.16 C).
//!
//! Mirror of the EMIR pack architecture
//! ([`crate::dq::dqi::compute_emir_dqi_pack`]) for SFTR
//! inputs. v0.16 ships **4 SFTR DQIs** drawn from the T2
//! (transaction state) layer of the auth.079 message :
//!
//! - `DQI_LOAN_VALUE_MISSING_SFTR`
//! - `DQI_LOAN_VALUE_STALE_SFTR`
//! - `DQI_COLLATERAL_VALUE_MISSING_SFTR`
//! - `DQI_TIM_REPORTING_LATE_SFTR`
//!
//! **Deferred to v0.17** (documented limitation) :
//! - T3-layer margin DQIs (IM/VM posted/received pre/post-
//!   haircut) — requires extending `SftrTrStateRecord` +
//!   the `auth.079` parser to project T3 fields. The T3
//!   stratum is described in
//!   `docs/data-quality-pack.md` "SFTR layer mapping".
//! - SFTR-specific DQIs (haircut anomaly, reuse untracked,
//!   per-CP LEI rollups, reconciliation-status from
//!   `auth.080`) — orthogonal to T3, scope-shaped for v0.17.

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
use crate::model::{DqDimension, DqIssue, Regime, SftrRecord, SftrTrStateRecord};
use crate::Thresholds;

/// All SFTR DQI inputs, each optional. v0.16 ships 4
/// indicators on top of the TSR + TAR layers ; reconciliation
/// (auth.080) and missing-collateral (auth.083) inputs are
/// **reserved** on the struct for v0.17 indicators (no
/// computer reads them yet).
#[derive(Debug, Default, Clone)]
pub struct SftrDqiInputs<'a> {
    /// SFTR Trade State Report (`auth.079`) — projected onto
    /// [`SftrTrStateRecord`].
    pub tsr: Option<&'a [SftrTrStateRecord]>,
    /// SFTR Trade Activity Report (`auth.052`) — projected
    /// onto [`SftrRecord`].
    pub tar: Option<&'a [SftrRecord]>,
    /// SFTR Reconciliation Status Advice (`auth.080`) —
    /// reserved input slot for v0.17 `DQI_REC_STATUS_*_SFTR`
    /// indicators. Not read by any v0.16 computer.
    pub reconciliation: Option<&'a [crate::ReconciliationRecord]>,
    /// SFTR Missing-Collateral request (`auth.083`) —
    /// reserved input slot for v0.17 `DQI_MCR_*` rollups.
    /// Not read by any v0.16 computer.
    pub missing_collateral: Option<&'a [crate::MissingCollateralRecord]>,
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
