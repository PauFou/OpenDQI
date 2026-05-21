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

mod emir_pack;
mod model;
mod thresholds;

pub mod compute;

pub use compute::{
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
pub use emir_pack::{compute_emir_dqi_pack, EmirDqiInputs};
pub use model::{DqiEvidence, DqiIndicator, DqiPackResult, DqiStatus, MappingPresence};
pub use thresholds::{compute_status, default_dqi_thresholds, DqiThresholdPair};
