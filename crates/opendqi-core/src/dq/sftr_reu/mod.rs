//! SFTR Reused Collateral Data Report (`auth.071`) granular
//! checks — v0.18 B5. Per-record defects on
//! [`crate::SftrReuseActivityRecord`] : completeness of the
//! reuse currency on records carrying amounts, accuracy of the
//! cash reinvestment rate against a plausible band.
//!
//! No `prior` argument on `run` — auth.071 events are evaluated
//! in isolation (v0.18 has no reuse-history lifecycle tracking;
//! cross-batch chain-depth analysis deferred to v0.19+).
//!
//! Granular checks complement the 2 v0.18 aggregate reuse DQIs
//! sourced from the same slice (B4) : the DQIs roll up the
//! rate, these per-record checks pinpoint the exact violating
//! row.

use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Severity, SftrReuseActivityRecord};

/// An SFTR auth.071 reuse-activity check.
pub trait SftrReuCheck: Send + Sync {
    /// Stable identifier, e.g. `SFTR.REU.MISSING_REUSE_CURRENCY`.
    fn id(&self) -> &'static str;
    /// The DQ dimension this check belongs to.
    fn dimension(&self) -> DqDimension;
    /// Default severity for issues raised by this check.
    fn severity(&self) -> Severity;
    /// Execute the check over the reuse-activity records slice.
    fn run(&self, records: &[SftrReuseActivityRecord], ctx: &CheckContext) -> Vec<DqIssue>;
}

mod missing_reuse_currency;
mod rate_outside_plausible_band;

pub use missing_reuse_currency::SftrReuMissingReuseCurrency;
pub use rate_outside_plausible_band::SftrReuRateOutsidePlausibleBand;
