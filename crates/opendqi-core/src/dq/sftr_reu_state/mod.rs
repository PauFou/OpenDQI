//! SFTR Reused Collateral Data Transaction State Report
//! (`auth.086`) granular checks — v0.18 C5. Per-record defects
//! on [`crate::SftrReuseStateRecord`] : completeness of the
//! reuse currency on records carrying amounts, accuracy of the
//! cash reinvestment rate against a plausible band.
//!
//! Sister module to `sftr_reu/` (auth.071 events). The two
//! check families are structurally identical because the
//! underlying field shape is identical (12 fields, same
//! semantics) — separated so the trait stays type-constrained
//! and registries don't mix activity- and state-record types.

use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Severity, SftrReuseStateRecord};

/// An SFTR auth.086 reuse-state check.
pub trait SftrReuStateCheck: Send + Sync {
    /// Stable identifier, e.g. `SFTR.REU.STATE.MISSING_REUSE_CURRENCY`.
    fn id(&self) -> &'static str;
    /// The DQ dimension this check belongs to.
    fn dimension(&self) -> DqDimension;
    /// Default severity for issues raised by this check.
    fn severity(&self) -> Severity;
    /// Execute the check over the reuse-state records slice.
    fn run(&self, records: &[SftrReuseStateRecord], ctx: &CheckContext) -> Vec<DqIssue>;
}

mod missing_reuse_currency;
mod rate_outside_plausible_band;

pub use missing_reuse_currency::SftrReuStateMissingReuseCurrency;
pub use rate_outside_plausible_band::SftrReuStateRateOutsidePlausibleBand;
