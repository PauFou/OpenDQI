//! SFTR Trade State Report (TSR) checks. State-oriented over the
//! TR's snapshot of outstanding SFTs (`auth.079`).

use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Severity, SftrRecord, SftrTrStateRecord};

/// A SFTR TSR check.
pub trait SftrTrStateCheck: Send + Sync {
    /// Stable identifier, e.g. `SFTR.TST.STALE_VALUATION`.
    fn id(&self) -> &'static str;
    /// The DQ dimension this check belongs to.
    fn dimension(&self) -> DqDimension;
    /// Default severity for issues raised by this check.
    fn severity(&self) -> Severity;
    /// Execute the check against the TSR snapshot + any prior SFTR
    /// records loaded from the history store.
    fn run(
        &self,
        records: &[SftrTrStateRecord],
        prior: &[SftrRecord],
        ctx: &CheckContext,
    ) -> Vec<DqIssue>;
}

mod active_past_maturity;
mod duplicate_active_uti;
mod haircut_out_of_range;
mod missing_collateral;
mod outstanding_summary;
mod stale_valuation;

pub use active_past_maturity::SftrActivePastMaturity;
pub use duplicate_active_uti::SftrDuplicateActiveUti;
pub use haircut_out_of_range::SftrHaircutOutOfRangeOnTsr;
pub use missing_collateral::SftrMissingCollateralOnTsr;
pub use outstanding_summary::SftrOutstandingSummary;
pub use stale_valuation::SftrStaleValuationOnTsr;

pub(crate) fn is_outstanding(r: &SftrTrStateRecord) -> bool {
    match r.status.as_deref() {
        None => true,
        Some(s) => {
            let s = s.trim();
            s.is_empty()
                || s.eq_ignore_ascii_case("OUTSTANDING")
                || s.eq_ignore_ascii_case("ACTIVE")
                || s.eq_ignore_ascii_case("LIVE")
        }
    }
}
