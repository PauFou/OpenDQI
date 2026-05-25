//! SFTR Margin Data Transaction Report (`auth.070`) granular
//! checks — v0.18 A5. Event-driven per-MAR-record defects on
//! [`crate::SftrMarginActivityRecord`] : validity of the event
//! date / action-type enum, completeness of the amount currency,
//! presence of the mandatory portfolio identifier.
//!
//! No `prior` argument on `run` — auth.070 events are evaluated
//! in isolation (v0.18 has no MAR-history lifecycle tracking;
//! cross-batch amount-change implausibility deferred to v0.19+).
//!
//! Granular checks complement the 3 v0.18 aggregate MAR DQIs
//! sourced from the same MAR slice (A4) : the DQIs roll up the
//! rate, these per-record checks pinpoint the exact violating row.
//!
//! Sister module : `sftr_msr/` (auth.085 portfolio-state checks).

use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Severity, SftrMarginActivityRecord};

/// An SFTR MAR check.
pub trait SftrMarCheck: Send + Sync {
    /// Stable identifier, e.g. `SFTR.MAR.EVENT_DATE_IN_FUTURE`.
    fn id(&self) -> &'static str;
    /// The DQ dimension this check belongs to.
    fn dimension(&self) -> DqDimension;
    /// Default severity for issues raised by this check.
    fn severity(&self) -> Severity;
    /// Execute the check over the MAR records slice.
    fn run(&self, records: &[SftrMarginActivityRecord], ctx: &CheckContext) -> Vec<DqIssue>;
}

mod action_type_enum_invalid;
mod amount_currency_missing;
mod event_date_in_future;
mod event_without_portfolio;

pub use action_type_enum_invalid::SftrMarActionTypeEnumInvalid;
pub use amount_currency_missing::SftrMarAmountCurrencyMissing;
pub use event_date_in_future::SftrMarEventDateInFuture;
pub use event_without_portfolio::SftrMarEventWithoutPortfolio;
