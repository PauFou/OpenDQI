//! SFTR pre-submission checks driven by a `RejectionProfile` —
//! mirror of the EMIR `pre_submission` family but operating on
//! `SftrRecord` slices. Closes the loop between post-TR feedback
//! (`auth.080`) analytics and the next `opendqi sftr scan`.

use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, RejectionProfile, Severity, SftrRecord};

mod likely_rejection_pattern;
mod repeated_rejection;

pub use likely_rejection_pattern::SftrPscLikelyRejectionPattern;
pub use repeated_rejection::SftrPscRepeatedRejection;

/// A pre-submission SFTR check. Operates against the current batch
/// plus a `RejectionProfile` loaded from disk.
pub trait SftrPreSubmissionCheck: Send + Sync {
    /// Stable identifier, e.g. `SFTR.PSC.REPEATED_REJECTION`.
    fn id(&self) -> &'static str;
    /// The DQ dimension this check belongs to.
    fn dimension(&self) -> DqDimension;
    /// Default severity for issues raised by this check.
    fn severity(&self) -> Severity;
    /// Execute the check against the current records + the profile.
    fn run(
        &self,
        records: &[SftrRecord],
        profile: &RejectionProfile,
        ctx: &CheckContext,
    ) -> Vec<DqIssue>;
}

/// Default SFTR pre-submission check registry (2 checks for v1).
pub fn default_sftr_pre_submission_checks() -> Vec<Box<dyn SftrPreSubmissionCheck>> {
    vec![
        Box::new(SftrPscRepeatedRejection),
        Box::new(SftrPscLikelyRejectionPattern),
    ]
}
