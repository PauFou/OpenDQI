//! EMIR Data Quality Pack orchestrator.
//!
//! [`compute_emir_dqi_pack`] takes the EMIR TR layers (TSR /
//! TAR / MSR / MAR / Feedback / recon_stats / reconciliation)
//! — each `Option<&[...]>` — and returns a [`DqiPackResult`]
//! bundling:
//!
//! - **14 [`DqiIndicator`]s in v0.16 B1** (10 v0.15 + 4 v0.16
//!   B1 cross-CP from auth.091). One row per shipped DQI, even
//!   when the input layer is absent — the indicator just
//!   carries [`DqiStatus::NotApplicable`]. Stable ordering by
//!   [`DqiIndicator::indicator_id`] for deterministic output.
//! - **Top-N [`DqiEvidence`]** per indicator (≤ 20 each).
//! - **Granular issues** from running the existing 216-check
//!   registries on every layer provided. The DQI pack does
//!   **not** replace the granular issue stream — it adds a
//!   committee-readable layer on top.
//! - A standard [`ScanSummary`] over the granular issues
//!   (via [`IssueAggregator`]).
//!
//! v0.16 grows the EMIR pack from 10 → 14+ indicators
//! incrementally across phases B1-B4. SFTR has its own
//! `compute_sftr_dqi_pack` (v0.16 C1+).

use chrono::{DateTime, NaiveDate, Utc};

use crate::dq::aggregate::IssueAggregator;
use crate::dq::dqi::compute::{
    compute_dqi_anomaly_rate, compute_dqi_col_all_zero, compute_dqi_col_missing_state,
    compute_dqi_col_stale_state, compute_dqi_conf_missing, compute_dqi_duplicate_reports,
    compute_dqi_err_missing, compute_dqi_field_mismatch_rate, compute_dqi_lei_missing,
    compute_dqi_margin_inconsistent_post_haircut, compute_dqi_margin_inconsistent_pre_haircut,
    compute_dqi_nature_missing, compute_dqi_notional_inconsistent, compute_dqi_pairing_rate,
    compute_dqi_rec_status_unpaired, compute_dqi_reconciliation_rate, compute_dqi_rej_rate,
    compute_dqi_rej_repeat_uti, compute_dqi_sector_missing, compute_dqi_tim_reporting_late,
    compute_dqi_unpaired_trades_rate, compute_dqi_val_missing, compute_dqi_val_stale,
    compute_dqi_vm_missing_for_cleared,
};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, DqiPackResult, MappingPresence};
use crate::dq::{
    default_checks, default_feedback_checks, default_margin_activity_checks,
    default_margin_state_checks, default_tr_state_checks, run_all, run_all_feedback,
    run_all_margin_activity, run_all_margin_state, run_all_tr_state, CheckContext,
};
use crate::model::{
    DqIssue, EmirRecord, FeedbackRecord, MarginActivityRecord, MarginStateRecord, Regime,
    TrStateRecord,
};
use crate::Thresholds;

/// All EMIR DQI inputs, each optional. v0.15 enforces no
/// minimum (an empty pack is legal — every indicator returns
/// `NotApplicable`); the CLI / Python layer enforces "at least
/// one input" upstream so users get an obvious error rather
/// than a silent empty pack.
#[derive(Debug, Default, Clone)]
pub struct EmirDqiInputs<'a> {
    /// Trade State Report (`auth.107`).
    pub tsr: Option<&'a [TrStateRecord]>,
    /// Trade Activity Report (`auth.030`) — modelled as
    /// [`EmirRecord`] like submission XML.
    pub tar: Option<&'a [EmirRecord]>,
    /// Margin State Report (`auth.109`).
    pub msr: Option<&'a [MarginStateRecord]>,
    /// Margin Activity Report (`auth.108`).
    pub mar: Option<&'a [MarginActivityRecord]>,
    /// Trade-Report Validation Feedback (`auth.092`).
    pub feedback: Option<&'a [FeedbackRecord]>,
    /// Reconciliation Statistics (`auth.091`) per-counterparty
    /// rates. v0.16+. Feeds `DQI_PAIRING_RATE` and
    /// `DQI_RECONCILIATION_RATE`.
    pub recon_stats: Option<&'a [crate::ReconStatsRecord]>,
    /// Per-transaction reconciliation records (also derived from
    /// `auth.091` per-tx detail). v0.16+. Feeds
    /// `DQI_UNPAIRED_TRADES_RATE`, `DQI_FIELD_MISMATCH_RATE`, and
    /// the v0.16 margin/notional consistency DQIs.
    pub reconciliation: Option<&'a [crate::ReconciliationRecord]>,
}

/// Run the full EMIR Data Quality Pack.
///
/// `as_of` is the reference date used by every age-based DQI
/// (stale valuation, stale collateral state). When the caller
/// has no specific snapshot date, `Utc::now().date_naive()` is
/// the conventional default — the CLI / Python layer defaults
/// to today.
///
/// Returns a [`DqiPackResult`] with all 14 EMIR indicators
/// (10 v0.15 + 4 v0.16 B1 cross-CP from auth.091), in
/// stable `indicator_id` order, their evidence rows, the
/// granular issues from running the 216-check registries on
/// every provided layer, and a [`ScanSummary`] over those
/// issues.
///
/// **No I/O, no parsing** — pure function. The CLI and Python
/// layers parse the input files into the typed record vectors
/// and pass slices in.
pub fn compute_emir_dqi_pack(
    inputs: EmirDqiInputs<'_>,
    mapping_presence: MappingPresence,
    thresholds: &Thresholds,
    as_of: NaiveDate,
) -> DqiPackResult {
    let started_at = Utc::now();

    // ---------- DQI layer ----------
    // v0.15 shipped 10 EMIR DQIs ; v0.16 grows to 14 with B1
    // (auth.091 cross-CP) ; B3+B4 will add more.
    let mut indicators: Vec<DqiIndicator> = Vec::with_capacity(14);
    let mut evidence: Vec<DqiEvidence> = Vec::new();

    let mut push = |ind: DqiIndicator, mut ev: Vec<DqiEvidence>| {
        indicators.push(ind);
        evidence.append(&mut ev);
    };

    // TSR-only indicators (3 v0.15 + 1 v0.16 B3 + 2 v0.16 B4):
    // VAL_MISSING, VAL_STALE, LEI_MISSING, ANOMALY_RATE,
    // DUPLICATE_REPORTS ; contributes to COL_MISSING_STATE.
    if let Some(tsr) = inputs.tsr {
        let (ind, ev) = compute_dqi_val_missing(tsr, thresholds, mapping_presence);
        push(ind, ev);
        let (ind, ev) = compute_dqi_val_stale(tsr, thresholds, as_of, mapping_presence);
        push(ind, ev);
        let (ind, ev) = compute_dqi_lei_missing(tsr, thresholds, mapping_presence);
        push(ind, ev);
        let (ind, ev) = compute_dqi_anomaly_rate(tsr, thresholds, mapping_presence);
        push(ind, ev);
        let (ind, ev) = compute_dqi_duplicate_reports(tsr, thresholds, mapping_presence);
        push(ind, ev);
    } else {
        push(
            not_applicable("DQI_VAL_MISSING", "TSR not provided"),
            Vec::new(),
        );
        push(
            not_applicable("DQI_VAL_STALE", "TSR not provided"),
            Vec::new(),
        );
        push(
            not_applicable("DQI_LEI_MISSING", "TSR not provided"),
            Vec::new(),
        );
        push(
            not_applicable("DQI_ANOMALY_RATE", "TSR not provided"),
            Vec::new(),
        );
        push(
            not_applicable("DQI_DUPLICATE_REPORTS", "TSR not provided"),
            Vec::new(),
        );
    }

    // MSR-only indicators (2 v0.15 + 1 v0.16 B4): COL_ALL_ZERO,
    // COL_STALE_STATE, VM_MISSING_FOR_CLEARED.
    if let Some(msr) = inputs.msr {
        let (ind, ev) = compute_dqi_col_all_zero(msr, thresholds, mapping_presence);
        push(ind, ev);
        let (ind, ev) = compute_dqi_col_stale_state(msr, thresholds, as_of, mapping_presence);
        push(ind, ev);
        let (ind, ev) = compute_dqi_vm_missing_for_cleared(msr, thresholds, mapping_presence);
        push(ind, ev);
    } else {
        push(
            not_applicable("DQI_COL_ALL_ZERO", "MSR not provided"),
            Vec::new(),
        );
        push(
            not_applicable("DQI_COL_STALE_STATE", "MSR not provided"),
            Vec::new(),
        );
        push(
            not_applicable("DQI_VM_MISSING_FOR_CLEARED", "MSR not provided"),
            Vec::new(),
        );
    }

    // TSR + MSR cross-table indicator (1): COL_MISSING_STATE.
    match (inputs.tsr, inputs.msr) {
        (Some(tsr), Some(msr)) => {
            let (ind, ev) = compute_dqi_col_missing_state(tsr, msr, thresholds, mapping_presence);
            push(ind, ev);
        }
        _ => push(
            not_applicable(
                "DQI_COL_MISSING_STATE",
                "TSR + MSR both required for cross-table indicator",
            ),
            Vec::new(),
        ),
    }

    // Feedback-only indicators (2): REJ_RATE, REJ_REPEAT_UTI.
    if let Some(feedback) = inputs.feedback {
        let (ind, ev) = compute_dqi_rej_rate(feedback, thresholds, mapping_presence);
        push(ind, ev);
        let (ind, ev) = compute_dqi_rej_repeat_uti(feedback, thresholds, mapping_presence);
        push(ind, ev);
    } else {
        push(
            not_applicable("DQI_REJ_RATE", "Feedback not provided"),
            Vec::new(),
        );
        push(
            not_applicable("DQI_REJ_REPEAT_UTI", "Feedback not provided"),
            Vec::new(),
        );
    }

    // TAR-only indicators (3 v0.15 + 3 v0.16 B3 presence):
    // TIM_REPORTING_LATE, CONF_MISSING (gated),
    // REC_STATUS_UNPAIRED (gated), ERR_MISSING, NATURE_MISSING,
    // SECTOR_MISSING.
    if let Some(tar) = inputs.tar {
        let (ind, ev) = compute_dqi_tim_reporting_late(tar, thresholds, mapping_presence);
        push(ind, ev);
        let (ind, ev) = compute_dqi_conf_missing(tar, thresholds, mapping_presence);
        push(ind, ev);
        let (ind, ev) = compute_dqi_rec_status_unpaired(tar, thresholds, mapping_presence);
        push(ind, ev);
        let (ind, ev) = compute_dqi_err_missing(tar, thresholds, mapping_presence);
        push(ind, ev);
        let (ind, ev) = compute_dqi_nature_missing(tar, thresholds, mapping_presence);
        push(ind, ev);
        let (ind, ev) = compute_dqi_sector_missing(tar, thresholds, mapping_presence);
        push(ind, ev);
    } else {
        push(
            not_applicable("DQI_TIM_REPORTING_LATE", "TAR not provided"),
            Vec::new(),
        );
        push(
            not_applicable("DQI_CONF_MISSING", "TAR not provided"),
            Vec::new(),
        );
        push(
            not_applicable("DQI_REC_STATUS_UNPAIRED", "TAR not provided"),
            Vec::new(),
        );
        push(
            not_applicable("DQI_ERR_MISSING", "TAR not provided"),
            Vec::new(),
        );
        push(
            not_applicable("DQI_NATURE_MISSING", "TAR not provided"),
            Vec::new(),
        );
        push(
            not_applicable("DQI_SECTOR_MISSING", "TAR not provided"),
            Vec::new(),
        );
    }

    // v0.16 B1 — auth.091-derived cross-CP indicators (4).
    // recon_stats (counterparty rates) feeds 2 ; reconciliation
    // (per-trade detail) feeds 2.
    if let Some(rs) = inputs.recon_stats {
        let (ind, ev) = compute_dqi_pairing_rate(rs, thresholds, mapping_presence);
        push(ind, ev);
        let (ind, ev) = compute_dqi_reconciliation_rate(rs, thresholds, mapping_presence);
        push(ind, ev);
    } else {
        push(
            not_applicable("DQI_PAIRING_RATE", "auth.091 recon_stats not provided"),
            Vec::new(),
        );
        push(
            not_applicable(
                "DQI_RECONCILIATION_RATE",
                "auth.091 recon_stats not provided",
            ),
            Vec::new(),
        );
    }
    if let Some(rr) = inputs.reconciliation {
        let (ind, ev) = compute_dqi_unpaired_trades_rate(rr, thresholds, mapping_presence);
        push(ind, ev);
        let (ind, ev) = compute_dqi_field_mismatch_rate(rr, thresholds, mapping_presence);
        push(ind, ev);
        // v0.16 B2 — per-criterion mismatch DQIs (3).
        let (ind, ev) = compute_dqi_notional_inconsistent(rr, thresholds, mapping_presence);
        push(ind, ev);
        let (ind, ev) =
            compute_dqi_margin_inconsistent_pre_haircut(rr, thresholds, mapping_presence);
        push(ind, ev);
        let (ind, ev) =
            compute_dqi_margin_inconsistent_post_haircut(rr, thresholds, mapping_presence);
        push(ind, ev);
    } else {
        push(
            not_applicable(
                "DQI_UNPAIRED_TRADES_RATE",
                "auth.091 per-tx reconciliation records not provided",
            ),
            Vec::new(),
        );
        push(
            not_applicable(
                "DQI_FIELD_MISMATCH_RATE",
                "auth.091 per-tx reconciliation records not provided",
            ),
            Vec::new(),
        );
        push(
            not_applicable(
                "DQI_NOTIONAL_INCONSISTENT",
                "auth.091 per-tx reconciliation records not provided",
            ),
            Vec::new(),
        );
        push(
            not_applicable(
                "DQI_MARGIN_INCONSISTENT_PRE_HAIRCUT",
                "auth.091 per-tx reconciliation records not provided",
            ),
            Vec::new(),
        );
        push(
            not_applicable(
                "DQI_MARGIN_INCONSISTENT_POST_HAIRCUT",
                "auth.091 per-tx reconciliation records not provided",
            ),
            Vec::new(),
        );
    }

    // Stable order: sort by indicator_id ascending so downstream
    // outputs are deterministic regardless of the dispatch order
    // above.
    indicators.sort_by(|a, b| a.indicator_id.cmp(&b.indicator_id));

    // ---------- granular issues layer ----------
    let ctx = build_ctx(thresholds.clone(), as_of);
    let mut all_issues: Vec<DqIssue> = Vec::new();
    let mut files_processed: u32 = 0;
    let mut records_processed: u32 = 0;

    if let Some(tar) = inputs.tar {
        files_processed += 1;
        records_processed = records_processed.saturating_add(tar.len() as u32);
        all_issues.append(&mut run_all(&default_checks(), tar, &ctx));
    }
    if let Some(tsr) = inputs.tsr {
        files_processed += 1;
        records_processed = records_processed.saturating_add(tsr.len() as u32);
        let prior: &[EmirRecord] = &[];
        all_issues.append(&mut run_all_tr_state(
            &default_tr_state_checks(),
            tsr,
            prior,
            &ctx,
        ));
    }
    if let Some(msr) = inputs.msr {
        files_processed += 1;
        records_processed = records_processed.saturating_add(msr.len() as u32);
        let prior: &[EmirRecord] = &[];
        all_issues.append(&mut run_all_margin_state(
            &default_margin_state_checks(),
            msr,
            prior,
            &ctx,
        ));
    }
    if let Some(mar) = inputs.mar {
        files_processed += 1;
        records_processed = records_processed.saturating_add(mar.len() as u32);
        let prior: &[MarginActivityRecord] = &[];
        all_issues.append(&mut run_all_margin_activity(
            &default_margin_activity_checks(),
            mar,
            prior,
            &ctx,
        ));
    }
    if let Some(feedback) = inputs.feedback {
        files_processed += 1;
        records_processed = records_processed.saturating_add(feedback.len() as u32);
        let prior: &[EmirRecord] = &[];
        all_issues.append(&mut run_all_feedback(
            &default_feedback_checks(),
            feedback,
            prior,
            &ctx,
        ));
    }

    let aggregator = IssueAggregator::from_issues(&all_issues);
    let finished_at = Utc::now();
    let issues_summary = aggregator.into_summary(
        Regime::Emir,
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
    use crate::dq::dqi::DqiStatus;
    use crate::model::DqDimension;

    DqiIndicator {
        indicator_id: indicator_id.into(),
        regime: Regime::Emir,
        // We don't know the dimension/scope here without a layer
        // context — pick defaults that make sense for the
        // "unspecified" case. Downstream consumers should treat
        // NotApplicable indicators as non-actionable.
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
    use std::collections::BTreeMap;

    use rust_decimal::Decimal;

    use super::*;
    use crate::dq::dqi::DqiStatus;
    use crate::model::FeedbackType;

    fn as_of() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 5, 21).unwrap()
    }

    fn tsr_row(uti: &str, val: Option<Decimal>, status: &str) -> TrStateRecord {
        TrStateRecord {
            uti: Some(uti.into()),
            status: Some(status.into()),
            valuation_amount: val,
            ..Default::default()
        }
    }

    fn tar_row(uti: &str) -> EmirRecord {
        EmirRecord {
            uti: Some(uti.into()),
            ..Default::default()
        }
    }

    fn msr_row(uti: &str) -> MarginStateRecord {
        MarginStateRecord {
            uti: Some(uti.into()),
            ..Default::default()
        }
    }

    fn rejected(uti: &str) -> FeedbackRecord {
        FeedbackRecord {
            uti: Some(uti.into()),
            feedback_type: FeedbackType::Rejected,
            ..Default::default()
        }
    }

    #[test]
    fn empty_inputs_yield_all_not_applicable_indicators() {
        let result = compute_emir_dqi_pack(
            EmirDqiInputs::default(),
            MappingPresence::default(),
            &Thresholds::default(),
            as_of(),
        );
        assert_eq!(
            result.indicators.len(),
            24,
            "always exactly 24 indicators in v0.16 B4 (10 v0.15 + 4 B1 + 3 B2 + 4 B3 + 3 B4 final)"
        );
        for ind in &result.indicators {
            assert_eq!(
                ind.status,
                DqiStatus::NotApplicable,
                "{} should be NotApplicable",
                ind.indicator_id
            );
        }
        assert!(result.evidence.is_empty());
        assert_eq!(result.issues_summary.files_processed, 0);
        assert_eq!(result.issues_summary.records_processed, 0);
        assert_eq!(result.issues_summary.issues_total, 0);
        assert_eq!(result.issues_summary.quality_score, 100.0);
    }

    #[test]
    fn indicators_sorted_alphabetically() {
        let result = compute_emir_dqi_pack(
            EmirDqiInputs::default(),
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
        // And confirm we have all the v0.16 B1 IDs.
        assert_eq!(
            ids,
            vec![
                "DQI_ANOMALY_RATE",
                "DQI_COL_ALL_ZERO",
                "DQI_COL_MISSING_STATE",
                "DQI_COL_STALE_STATE",
                "DQI_CONF_MISSING",
                "DQI_DUPLICATE_REPORTS",
                "DQI_ERR_MISSING",
                "DQI_FIELD_MISMATCH_RATE",
                "DQI_LEI_MISSING",
                "DQI_MARGIN_INCONSISTENT_POST_HAIRCUT",
                "DQI_MARGIN_INCONSISTENT_PRE_HAIRCUT",
                "DQI_NATURE_MISSING",
                "DQI_NOTIONAL_INCONSISTENT",
                "DQI_PAIRING_RATE",
                "DQI_RECONCILIATION_RATE",
                "DQI_REC_STATUS_UNPAIRED",
                "DQI_REJ_RATE",
                "DQI_REJ_REPEAT_UTI",
                "DQI_SECTOR_MISSING",
                "DQI_TIM_REPORTING_LATE",
                "DQI_UNPAIRED_TRADES_RATE",
                "DQI_VAL_MISSING",
                "DQI_VAL_STALE",
                "DQI_VM_MISSING_FOR_CLEARED",
            ]
        );
    }

    #[test]
    fn tsr_only_pack_runs_3_indicators_and_granular_tsr_checks() {
        let tsr = vec![
            tsr_row("U1", Some(Decimal::new(100, 0)), "OUTSTANDING"),
            tsr_row("U2", None, "OUTSTANDING"), // missing valuation
        ];
        let inputs = EmirDqiInputs {
            tsr: Some(&tsr),
            ..Default::default()
        };
        let result = compute_emir_dqi_pack(
            inputs,
            MappingPresence::default(),
            &Thresholds::default(),
            as_of(),
        );
        // 10 indicators always; 3 of them computed (VAL_MISSING,
        // VAL_STALE, COL_MISSING_STATE -> still NA because no
        // collateral portfolio code → denominator zero).
        assert_eq!(result.indicators.len(), 24);
        let val_missing = result
            .indicators
            .iter()
            .find(|i| i.indicator_id == "DQI_VAL_MISSING")
            .unwrap();
        assert_eq!(val_missing.denominator, 2);
        assert_eq!(val_missing.numerator, 1);
        // Granular issues from default_tr_state_checks should be ≥ 1
        // (at least the missing-valuation row gets flagged).
        assert_eq!(result.issues_summary.files_processed, 1);
        assert_eq!(result.issues_summary.records_processed, 2);
        assert!(
            result.issues_summary.issues_total >= 1,
            "expected at least one granular issue, got {}",
            result.issues_summary.issues_total
        );
    }

    #[test]
    fn full_pack_with_all_5_layers() {
        let tsr = vec![tsr_row("U1", Some(Decimal::new(100, 0)), "OUTSTANDING")];
        let tar = vec![tar_row("U1"), tar_row("U2")];
        let msr = vec![msr_row("U1")];
        let mar: Vec<MarginActivityRecord> = Vec::new();
        let feedback = vec![rejected("U1"), rejected("U1")];

        let inputs = EmirDqiInputs {
            tsr: Some(&tsr),
            tar: Some(&tar),
            msr: Some(&msr),
            mar: Some(&mar),
            feedback: Some(&feedback),
            ..Default::default()
        };
        let result = compute_emir_dqi_pack(
            inputs,
            MappingPresence::default(),
            &Thresholds::default(),
            as_of(),
        );
        assert_eq!(result.indicators.len(), 24);
        // Feedback indicators must be computed now.
        let rej_rate = result
            .indicators
            .iter()
            .find(|i| i.indicator_id == "DQI_REJ_RATE")
            .unwrap();
        assert_eq!(rej_rate.denominator, 2);
        assert_eq!(rej_rate.numerator, 2);
        let rej_repeat = result
            .indicators
            .iter()
            .find(|i| i.indicator_id == "DQI_REJ_REPEAT_UTI")
            .unwrap();
        assert_eq!(rej_repeat.denominator, 1); // distinct rejected UTIs
        assert_eq!(rej_repeat.numerator, 1); // U1 rejected twice
        assert_eq!(result.issues_summary.files_processed, 5);
    }

    #[test]
    fn gated_indicators_with_mapping_presence_on() {
        // TAR with confirmation_timestamp + reconciliation_status
        // in raw_fields → gated indicators should compute.
        let mut tar_row = EmirRecord {
            uti: Some("U1".into()),
            ..Default::default()
        };
        let mut raw = BTreeMap::new();
        raw.insert(
            "confirmation_timestamp".into(),
            "2026-05-20T09:00:00Z".into(),
        );
        raw.insert("reconciliation_status".into(), "paired".into());
        tar_row.raw_fields = raw;
        let tar = vec![tar_row];

        let presence = MappingPresence {
            has_confirmation_timestamp: true,
            has_reconciliation_status: true,
        };
        let inputs = EmirDqiInputs {
            tar: Some(&tar),
            ..Default::default()
        };
        let result = compute_emir_dqi_pack(inputs, presence, &Thresholds::default(), as_of());

        let conf = result
            .indicators
            .iter()
            .find(|i| i.indicator_id == "DQI_CONF_MISSING")
            .unwrap();
        assert_eq!(conf.status, DqiStatus::Green);
        assert_eq!(conf.denominator, 1);
        assert_eq!(conf.numerator, 0);

        let rec = result
            .indicators
            .iter()
            .find(|i| i.indicator_id == "DQI_REC_STATUS_UNPAIRED")
            .unwrap();
        assert_eq!(rec.status, DqiStatus::Green);
    }
}
