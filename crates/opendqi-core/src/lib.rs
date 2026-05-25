//! OpenDQI core domain model and data-quality engine.
//!
//! This crate is intentionally framework-free. It exposes:
//!
//! - the canonical domain model (`Regime`, `Severity`, `DqDimension`,
//!   `DqIssue`, `EmirRecord`, `ScanSummary`);
//! - a `Thresholds` configuration struct;
//! - a `Check` trait and a registry of MVP EMIR checks via
//!   [`dq::default_checks`];
//! - a deterministic scoring function (see [`scoring::quality_score`]).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod business_days;
pub mod config;
pub mod dq;
pub mod model;
pub mod scoring;

pub use config::Thresholds;
pub use dq::dqi::{
    compute_dqi_anomaly_rate, compute_dqi_collateral_value_missing_sftr,
    compute_dqi_duplicate_reports, compute_dqi_err_missing, compute_dqi_field_mismatch_rate,
    compute_dqi_field_mismatch_rate_sftr, compute_dqi_haircut_anomaly_sftr,
    compute_dqi_lei_missing, compute_dqi_lei_missing_sftr, compute_dqi_loan_value_missing_sftr,
    compute_dqi_loan_value_stale_sftr, compute_dqi_margin_inconsistent_post_haircut,
    compute_dqi_margin_inconsistent_pre_haircut, compute_dqi_mcr_open_requests_sftr,
    compute_dqi_nature_missing, compute_dqi_notional_inconsistent, compute_dqi_pairing_rate,
    compute_dqi_pairing_rate_sftr, compute_dqi_reconciliation_rate,
    compute_dqi_reconciliation_rate_sftr, compute_dqi_sector_missing,
    compute_dqi_t3_excess_collateral_use_sftr, compute_dqi_t3_margin_posted_missing_sftr,
    compute_dqi_t3_margin_received_missing_sftr, compute_dqi_t3_margin_stale_sftr,
    compute_dqi_tim_reporting_late_sftr, compute_dqi_under_collateralization_sftr,
    compute_dqi_unpaired_trades_rate, compute_dqi_unpaired_trades_rate_sftr,
    compute_dqi_vm_missing_for_cleared, compute_emir_dqi_pack, compute_sftr_dqi_pack,
    compute_status, default_dqi_thresholds, DqiEvidence, DqiIndicator, DqiPackResult, DqiStatus,
    DqiThresholdPair, EmirDqiInputs, MappingPresence, SftrDqiInputs,
};
pub use dq::{
    default_checks, default_sftr_checks, default_sftr_mar_checks, default_sftr_msr_checks,
    default_sftr_reu_checks, default_sftr_reu_state_checks, stream_checks_into,
    stream_emir_checks_into, Check, IssueAggregator, SftrCheck, SftrMarCheck, SftrMsrCheck,
    SftrReuCheck, SftrReuStateCheck, SortedIssueSink, SortedIssues, STREAM_SPILL_MAX_ISSUES,
};
pub use model::{
    DqDimension, DqIssue, EmirPositionSetRecord, EmirRecord, EvidenceItem, FeedbackRecord,
    FeedbackType, MarginActivityRecord, MarginStateRecord, MissingCollateralRecord,
    ReconStatsRecord, ReconciliationRecord, Regime, RejectionCause, RejectionProfile,
    RejectionProfileFile, RepeatedRejection, ScanSummary, Severity, SftrMarginActivityRecord,
    SftrMarginStateRecord, SftrRecord, SftrReuseActivityRecord, SftrReuseStateRecord,
    SftrTrStateRecord, SftrTrStatusAdviceRecord, TrActivitySummary, TrStateRecord,
    TradeWarningsRecord, WarningsCounterpartyRecord, WarningsTransactionRecord,
};
pub use scoring::{quality_score, quality_score_from_counts, rate_with_status};
