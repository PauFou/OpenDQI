//! SFTR feedback (TR → firm) checks. Mirror of `dq::feedback` for the
//! SFTR regime.

use std::collections::HashSet;

use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, FeedbackRecord, Severity, SftrRecord};

/// A feedback (TR → firm) SFTR check.
pub trait SftrFeedbackCheck: Send + Sync {
    /// Stable identifier, e.g. `SFTR.FBK.TR_REJECTED_UTI`.
    fn id(&self) -> &'static str;
    /// The DQ dimension this check belongs to.
    fn dimension(&self) -> DqDimension;
    /// Default severity for issues raised by this check.
    fn severity(&self) -> Severity;
    /// Execute the check against feedback records + prior SFTR records.
    fn run(
        &self,
        feedback: &[FeedbackRecord],
        prior: &[SftrRecord],
        ctx: &CheckContext,
    ) -> Vec<DqIssue>;
}

mod tr_inaccurate_reported;
mod tr_missing_but_not_sent;
mod tr_missing_despite_submission;
mod tr_rejected_uti;

pub use tr_inaccurate_reported::SftrTrInaccurateReported;
pub use tr_missing_but_not_sent::SftrTrMissingButNotSent;
pub use tr_missing_despite_submission::SftrTrMissingDespiteSubmission;
pub use tr_rejected_uti::SftrTrRejectedUti;

pub(crate) fn priors_with_newt(prior: &[SftrRecord]) -> HashSet<&str> {
    let mut out: HashSet<&str> = HashSet::new();
    for r in prior {
        let is_newt = r
            .action_type
            .as_deref()
            .map(|a| a.eq_ignore_ascii_case("NEWT"))
            .unwrap_or(false);
        if !is_newt {
            continue;
        }
        if let Some(uti) = r.uti.as_deref() {
            let trimmed = uti.trim();
            if !trimmed.is_empty() {
                out.insert(trimmed);
            }
        }
    }
    out
}
