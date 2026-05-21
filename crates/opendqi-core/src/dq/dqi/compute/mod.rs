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

pub mod col_all_zero;
pub mod col_missing_state;
pub mod col_stale_state;
pub mod conf_missing;
pub mod rec_status_unpaired;
pub mod rej_rate;
pub mod rej_repeat_uti;
pub mod tim_reporting_late;
pub mod val_missing;
pub mod val_stale;

pub use col_all_zero::compute_dqi_col_all_zero;
pub use col_missing_state::compute_dqi_col_missing_state;
pub use col_stale_state::compute_dqi_col_stale_state;
pub use conf_missing::compute_dqi_conf_missing;
pub use rec_status_unpaired::compute_dqi_rec_status_unpaired;
pub use rej_rate::compute_dqi_rej_rate;
pub use rej_repeat_uti::compute_dqi_rej_repeat_uti;
pub use tim_reporting_late::compute_dqi_tim_reporting_late;
pub use val_missing::compute_dqi_val_missing;
pub use val_stale::compute_dqi_val_stale;

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
