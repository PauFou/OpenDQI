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
pub(crate) mod criterion_mismatch_rate;
pub mod err_missing;
pub mod field_mismatch_rate;
pub mod lei_missing;
pub mod margin_inconsistent_post_haircut;
pub mod margin_inconsistent_pre_haircut;
pub mod nature_missing;
pub mod notional_inconsistent;
pub mod pairing_rate;
pub mod rec_status_unpaired;
pub mod reconciliation_rate;
pub mod rej_rate;
pub mod rej_repeat_uti;
pub mod sector_missing;
pub mod tim_reporting_late;
pub mod unpaired_trades_rate;
pub mod val_missing;
pub mod val_stale;

pub use col_all_zero::compute_dqi_col_all_zero;
pub use col_missing_state::compute_dqi_col_missing_state;
pub use col_stale_state::compute_dqi_col_stale_state;
pub use conf_missing::compute_dqi_conf_missing;
pub use err_missing::compute_dqi_err_missing;
pub use field_mismatch_rate::compute_dqi_field_mismatch_rate;
pub use lei_missing::compute_dqi_lei_missing;
pub use margin_inconsistent_post_haircut::compute_dqi_margin_inconsistent_post_haircut;
pub use margin_inconsistent_pre_haircut::compute_dqi_margin_inconsistent_pre_haircut;
pub use nature_missing::compute_dqi_nature_missing;
pub use notional_inconsistent::compute_dqi_notional_inconsistent;
pub use pairing_rate::compute_dqi_pairing_rate;
pub use rec_status_unpaired::compute_dqi_rec_status_unpaired;
pub use reconciliation_rate::compute_dqi_reconciliation_rate;
pub use rej_rate::compute_dqi_rej_rate;
pub use rej_repeat_uti::compute_dqi_rej_repeat_uti;
pub use sector_missing::compute_dqi_sector_missing;
pub use tim_reporting_late::compute_dqi_tim_reporting_late;
pub use unpaired_trades_rate::compute_dqi_unpaired_trades_rate;
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
