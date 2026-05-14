//! Pre-submission checks driven by a `RejectionProfile` — close the
//! loop between post-TR feedback (`auth.092`) analytics and the
//! pre-submission scan.
//!
//! Each check sees the current EMIR records plus the rejection
//! profile loaded from `rejection_profile.yml` (produced by
//! `opendqi feedback analytics`), and surfaces issues that warn the
//! firm a record is likely to be rejected by the TR based on patterns
//! the TR has already flagged.

use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, EmirRecord, RejectionProfile, Severity};

mod likely_rejection_pattern;
mod repeated_rejection;

pub use likely_rejection_pattern::EmirPscLikelyRejectionPattern;
pub use repeated_rejection::EmirPscRepeatedRejection;

/// A pre-submission EMIR check. Operates against the current batch
/// plus a `RejectionProfile` loaded from disk.
pub trait PreSubmissionCheck: Send + Sync {
    /// Stable identifier, e.g. `EMIR.PSC.REPEATED_REJECTION`.
    fn id(&self) -> &'static str;
    /// The DQ dimension this check belongs to.
    fn dimension(&self) -> DqDimension;
    /// Default severity for issues raised by this check.
    fn severity(&self) -> Severity;
    /// Execute the check against the current records + the profile.
    fn run(
        &self,
        records: &[EmirRecord],
        profile: &RejectionProfile,
        ctx: &CheckContext,
    ) -> Vec<DqIssue>;
}

/// Default EMIR pre-submission check registry (2 checks for v1).
pub fn default_pre_submission_checks() -> Vec<Box<dyn PreSubmissionCheck>> {
    vec![
        Box::new(EmirPscRepeatedRejection),
        Box::new(EmirPscLikelyRejectionPattern),
    ]
}
