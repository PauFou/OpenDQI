//! Data Quality Pack — aggregated DQ indicators with
//! numerator / denominator / rate / threshold / status.
//!
//! Sits **above** the granular [`crate::dq::Check`] layer: the
//! 216 checks keep emitting [`crate::DqIssue`]s for forensic
//! drill-down, while this module rolls up regulator-style
//! [`DqiIndicator`]s a committee can read in 30 seconds.
//!
//! v0.15 ships 10 EMIR indicators across 4 dimensions
//! (completeness, timeliness, accuracy, consistency). SFTR
//! mirror is scheduled for v0.16.

mod model;
mod thresholds;

pub use model::{DqiEvidence, DqiIndicator, DqiPackResult, DqiStatus, MappingPresence};
pub use thresholds::{compute_status, default_dqi_thresholds, DqiThresholdPair};
