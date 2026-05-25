//! Per-DQI computers — each takes its input layer(s),
//! [`crate::Thresholds`], and an `as_of` reference date, and
//! returns `(DqiIndicator, Vec<DqiEvidence>)` with the
//! evidence already truncated to **top-20** by indicator-
//! specific priority (oldest first for `*_STALE`, biggest
//! delay first for `*_LATE`, etc.).
//!
//! v0.15 ships 10 computers:
//! - 5 single-table (this module): `val_missing`, `val_stale`,
//!   `col_all_zero`, `col_stale_state`, `tim_reporting_late`
//! - 3 cross-table + 2 gated (D3): `col_missing_state`,
//!   `rej_rate`, `rej_repeat_uti`, `conf_missing`,
//!   `rec_status_unpaired`

pub(crate) const EVIDENCE_TOP_N: usize = 20;

pub mod anomaly_rate;
pub mod col_all_zero;
pub mod col_missing_state;
pub mod col_stale_state;
pub mod collateral_value_missing_sftr;
pub mod conf_missing;
pub(crate) mod criterion_mismatch_rate;
pub mod duplicate_reports;
pub mod err_missing;
pub mod field_mismatch_rate;
pub mod field_mismatch_rate_sftr;
pub mod haircut_anomaly_sftr;
pub mod lei_missing;
pub mod lei_missing_sftr;
pub mod loan_value_missing_sftr;
pub mod loan_value_stale_sftr;
pub mod mar_event_spike_sftr;
pub mod mar_excess_collateral_event_rate_sftr;
pub mod mar_partial_sides_sftr;
pub mod margin_inconsistent_post_haircut;
pub mod margin_inconsistent_pre_haircut;
pub mod mcr_open_requests_sftr;
pub mod nature_missing;
pub mod notional_inconsistent;
pub mod pairing_rate;
pub mod pairing_rate_sftr;
pub mod rec_status_unpaired;
pub mod reconciliation_rate;
pub mod reconciliation_rate_sftr;
pub mod rej_rate;
pub mod rej_repeat_uti;
pub mod reuse_err_retraction_rate_sftr;
pub mod reuse_volume_missing_sftr;
pub mod sector_missing;
pub mod t3_excess_collateral_use_sftr;
pub mod t3_margin_posted_missing_sftr;
pub mod t3_margin_received_missing_sftr;
pub mod t3_margin_stale_sftr;
pub mod tim_reporting_late;
pub mod tim_reporting_late_sftr;
pub mod under_collateralization_sftr;
pub mod unpaired_trades_rate;
pub mod unpaired_trades_rate_sftr;
pub mod val_missing;
pub mod val_stale;
pub mod vm_missing_for_cleared;

pub use anomaly_rate::compute_dqi_anomaly_rate;
pub use col_all_zero::compute_dqi_col_all_zero;
pub use col_missing_state::compute_dqi_col_missing_state;
pub use col_stale_state::compute_dqi_col_stale_state;
pub use collateral_value_missing_sftr::compute_dqi_collateral_value_missing_sftr;
pub use conf_missing::compute_dqi_conf_missing;
pub use duplicate_reports::compute_dqi_duplicate_reports;
pub use err_missing::compute_dqi_err_missing;
pub use field_mismatch_rate::compute_dqi_field_mismatch_rate;
pub use field_mismatch_rate_sftr::compute_dqi_field_mismatch_rate_sftr;
pub use haircut_anomaly_sftr::compute_dqi_haircut_anomaly_sftr;
pub use lei_missing::compute_dqi_lei_missing;
pub use lei_missing_sftr::compute_dqi_lei_missing_sftr;
pub use loan_value_missing_sftr::compute_dqi_loan_value_missing_sftr;
pub use loan_value_stale_sftr::compute_dqi_loan_value_stale_sftr;
pub use mar_event_spike_sftr::compute_dqi_mar_event_spike_sftr;
pub use mar_excess_collateral_event_rate_sftr::compute_dqi_mar_excess_collateral_event_rate_sftr;
pub use mar_partial_sides_sftr::compute_dqi_mar_partial_sides_sftr;
pub use margin_inconsistent_post_haircut::compute_dqi_margin_inconsistent_post_haircut;
pub use margin_inconsistent_pre_haircut::compute_dqi_margin_inconsistent_pre_haircut;
pub use mcr_open_requests_sftr::compute_dqi_mcr_open_requests_sftr;
pub use nature_missing::compute_dqi_nature_missing;
pub use notional_inconsistent::compute_dqi_notional_inconsistent;
pub use pairing_rate::compute_dqi_pairing_rate;
pub use pairing_rate_sftr::compute_dqi_pairing_rate_sftr;
pub use rec_status_unpaired::compute_dqi_rec_status_unpaired;
pub use reconciliation_rate::compute_dqi_reconciliation_rate;
pub use reconciliation_rate_sftr::compute_dqi_reconciliation_rate_sftr;
pub use rej_rate::compute_dqi_rej_rate;
pub use rej_repeat_uti::compute_dqi_rej_repeat_uti;
pub use reuse_err_retraction_rate_sftr::compute_dqi_reuse_err_retraction_rate_sftr;
pub use reuse_volume_missing_sftr::compute_dqi_reuse_volume_missing_sftr;
pub use sector_missing::compute_dqi_sector_missing;
pub use t3_excess_collateral_use_sftr::compute_dqi_t3_excess_collateral_use_sftr;
pub use t3_margin_posted_missing_sftr::compute_dqi_t3_margin_posted_missing_sftr;
pub use t3_margin_received_missing_sftr::compute_dqi_t3_margin_received_missing_sftr;
pub use t3_margin_stale_sftr::compute_dqi_t3_margin_stale_sftr;
pub use tim_reporting_late::compute_dqi_tim_reporting_late;
pub use tim_reporting_late_sftr::compute_dqi_tim_reporting_late_sftr;
pub use under_collateralization_sftr::compute_dqi_under_collateralization_sftr;
pub use unpaired_trades_rate::compute_dqi_unpaired_trades_rate;
pub use unpaired_trades_rate_sftr::compute_dqi_unpaired_trades_rate_sftr;
pub use val_missing::compute_dqi_val_missing;
pub use val_stale::compute_dqi_val_stale;
pub use vm_missing_for_cleared::compute_dqi_vm_missing_for_cleared;

/// Internal helper: pick the [`crate::dq::dqi::DqiThresholdPair`]
/// for a given indicator ID from the user's [`crate::Thresholds`],
/// falling back first to the shipped defaults map and then to
/// the per-pair default.
///
/// This keeps each computer focused on counting — the resolution
/// chain (`user override` → `shipped default` → `loose default`)
/// is centralised here.
pub(crate) fn resolve_threshold(
    thresholds: &crate::Thresholds,
    indicator_id: &str,
) -> super::DqiThresholdPair {
    if let Some(t) = thresholds.dqi.get(indicator_id) {
        return *t;
    }
    super::default_dqi_thresholds()
        .get(indicator_id)
        .copied()
        .unwrap_or_default()
}
