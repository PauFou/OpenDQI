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

pub mod config;
pub mod dq;
pub mod model;
pub mod scoring;

pub use config::Thresholds;
pub use dq::{
    default_checks, default_sftr_checks, stream_emir_checks_into, Check, IssueAggregator,
    SftrCheck, SortedIssueSink, SortedIssues, STREAM_SPILL_MAX_ISSUES,
};
pub use model::{
    DqDimension, DqIssue, EmirRecord, EvidenceItem, FeedbackRecord, FeedbackType,
    MarginActivityRecord, MarginStateRecord, MissingCollateralRecord, ReconStatsRecord,
    ReconciliationRecord, Regime, RejectionCause, RejectionProfile, RejectionProfileFile,
    RepeatedRejection, ScanSummary, Severity, SftrRecord, SftrTrStateRecord, TrActivitySummary,
    TrStateRecord, TradeWarningsRecord, WarningsCounterpartyRecord, WarningsTransactionRecord,
};
pub use scoring::{quality_score, quality_score_from_counts};
