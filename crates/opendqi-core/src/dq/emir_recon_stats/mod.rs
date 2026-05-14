//! EMIR Reconciliation Statistics (auth.091) data-quality checks.
//!
//! Stat-oriented: each check ingests TR-produced pairing /
//! reconciliation rate summaries and surfaces thresholds breaches or
//! trends pointing to systemic reporting issues. Distinct from the
//! per-trade `EMIR.REC.*` family (auth.106 / reconciliation report).

use crate::config::ReconStatsThresholds;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, ReconStatsRecord, Regime, Severity};
use rust_decimal::prelude::ToPrimitive;

mod outstanding_unpaired_high;
mod pairing_rate_low;
mod pairing_rate_trend_down;
mod recon_rate_low;

pub use outstanding_unpaired_high::EmirRstOutstandingUnpairedHigh;
pub use pairing_rate_low::EmirRstPairingRateLow;
pub use pairing_rate_trend_down::EmirRstPairingRateTrendDown;
pub use recon_rate_low::EmirRstReconRateLow;

/// A reconciliation-statistics (auth.091) EMIR check. Takes the
/// current batch and the optional prior batch loaded from the
/// history store (used by the trend check).
pub trait ReconStatsCheck: Send + Sync {
    /// Stable identifier, e.g. `EMIR.RST.PAIRING_RATE_LOW`.
    fn id(&self) -> &'static str;
    /// The DQ dimension this check belongs to.
    fn dimension(&self) -> DqDimension;
    /// Default severity for issues raised by this check.
    fn severity(&self) -> Severity;
    /// Execute the check against the current + prior batches.
    fn run(
        &self,
        records: &[ReconStatsRecord],
        prior: &[ReconStatsRecord],
        ctx: &CheckContext,
    ) -> Vec<DqIssue>;
}

/// Default EMIR auth.091 check registry (4 checks).
pub fn default_recon_stats_checks() -> Vec<Box<dyn ReconStatsCheck>> {
    vec![
        Box::new(EmirRstPairingRateLow),
        Box::new(EmirRstReconRateLow),
        Box::new(EmirRstOutstandingUnpairedHigh),
        Box::new(EmirRstPairingRateTrendDown),
    ]
}

/// Decimal → f64 lossy converter; safe for rates in `[0.0, 1.0]`.
pub(crate) fn decimal_to_f64(d: rust_decimal::Decimal) -> f64 {
    d.to_f64().unwrap_or_default()
}

/// Format the counterparty label for issue messages: full LEI or
/// `(no counterparty)`.
pub(crate) fn counterparty_label(rec: &ReconStatsRecord) -> &str {
    rec.counterparty_lei.as_deref().unwrap_or("(no LEI)")
}

/// Format the reporting-date label for issue messages.
pub(crate) fn reporting_date_label(rec: &ReconStatsRecord) -> String {
    rec.reporting_date
        .map(|d| d.to_string())
        .unwrap_or_else(|| "(no date)".into())
}

/// Build a `DqIssue` shell pre-populated with common fields. Callers
/// override `field`, `value`, `message`.
pub(crate) fn build_issue(
    check_id: &str,
    severity: Severity,
    dimension: DqDimension,
    rec: &ReconStatsRecord,
) -> DqIssue {
    DqIssue {
        check_id: check_id.into(),
        regime: Regime::Emir,
        severity,
        dimension,
        record_id: rec.record_id.clone(),
        uti: None,
        field: None,
        value: None,
        message: String::new(),
        source_file: rec.source_file.clone(),
    }
}

/// Borrow the recon-stats thresholds from a CheckContext.
pub(crate) fn thresholds(ctx: &CheckContext) -> &ReconStatsThresholds {
    &ctx.thresholds.recon_stats
}
