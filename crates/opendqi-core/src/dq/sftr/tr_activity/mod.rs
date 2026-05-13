//! SFTR Trade Activity Report (TAR) checks. Activity-oriented over
//! `auth.052` ingested by `opendqi sftr scan` and replayed by the
//! TR. Same shape as EMIR.TRA but on `SftrRecord`.

use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Severity, SftrRecord, SftrTrStateRecord};

/// A SFTR TAR check.
pub trait SftrTrActivityCheck: Send + Sync {
    /// Stable identifier, e.g. `SFTR.TRA.SPIKE_TERM`.
    fn id(&self) -> &'static str;
    /// The DQ dimension this check belongs to.
    fn dimension(&self) -> DqDimension;
    /// Default severity for issues raised by this check.
    fn severity(&self) -> Severity;
    /// Execute the check against the TAR batch, prior submission
    /// history, and the optional companion SFTR TSR.
    fn run(
        &self,
        records: &[SftrRecord],
        prior: &[SftrRecord],
        tsr: Option<&[SftrTrStateRecord]>,
        ctx: &CheckContext,
    ) -> Vec<DqIssue>;
}

mod duplicate_newt_in_batch;
mod newt_not_in_tsr;
mod repeated_correction;
mod spike_modi;
mod spike_term;

pub use duplicate_newt_in_batch::SftrDuplicateNewtInBatch;
pub use newt_not_in_tsr::SftrNewtNotInTsr;
pub use repeated_correction::SftrRepeatedCorrection;
pub use spike_modi::SftrSpikeModi;
pub use spike_term::SftrSpikeTerm;
