//! SFTR Margin Data Transaction State Report (`auth.085`) granular
//! checks — v0.17 F1'. Portfolio-level margin state defects on
//! [`crate::SftrMarginStateRecord`] : partial-side completeness
//! (IM/VM posted/received missing despite other amounts set on
//! the same side), negative amounts, missing currency.
//!
//! No `prior` argument on `run` — auth.085 is portfolio-level and
//! v0.17 has no MSR lifecycle tracking (each report is a fresh
//! snapshot keyed by `collateral_portfolio_code`).
//!
//! Granular checks complement the 4 v0.17 aggregate DQIs sourced
//! from the same MSR slice : the DQIs roll up the rate, these
//! per-record checks pinpoint the exact violating row.

use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Severity, SftrMarginStateRecord};

/// An SFTR MSR check.
pub trait SftrMsrCheck: Send + Sync {
    /// Stable identifier, e.g. `SFTR.T3.IM_POSTED_MISSING`.
    fn id(&self) -> &'static str;
    /// The DQ dimension this check belongs to.
    fn dimension(&self) -> DqDimension;
    /// Default severity for issues raised by this check.
    fn severity(&self) -> Severity;
    /// Execute the check over the MSR records slice.
    fn run(&self, records: &[SftrMarginStateRecord], ctx: &CheckContext) -> Vec<DqIssue>;
}

mod im_posted_missing;
mod im_received_missing;
mod margin_currency_missing;
mod margin_negative;
mod vm_posted_missing;
mod vm_received_missing;

pub use im_posted_missing::SftrT3ImPostedMissing;
pub use im_received_missing::SftrT3ImReceivedMissing;
pub use margin_currency_missing::SftrT3MarginCurrencyMissing;
pub use margin_negative::SftrT3MarginNegative;
pub use vm_posted_missing::SftrT3VmPostedMissing;
pub use vm_received_missing::SftrT3VmReceivedMissing;
