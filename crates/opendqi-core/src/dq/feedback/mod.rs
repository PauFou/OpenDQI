//! EMIR feedback (TR → firm) checks. Each check sees the feedback
//! records parsed from an auth.092 message and the relevant prior
//! EMIR records loaded from the history store.

mod tr_inaccurate_reported;
mod tr_missing_but_not_sent;
mod tr_missing_despite_submission;
mod tr_rejected_uti;

pub use tr_inaccurate_reported::TrInaccurateReported;
pub use tr_missing_but_not_sent::TrMissingButNotSent;
pub use tr_missing_despite_submission::TrMissingDespiteSubmission;
pub use tr_rejected_uti::TrRejectedUti;

use std::collections::HashSet;

use crate::model::EmirRecord;

/// Returns the set of UTIs (case-trimmed) for which a NEWT action
/// exists in `prior`. Used by the missing-vs-sent checks to decide
/// whether the firm actually submitted the report.
pub(crate) fn priors_with_newt(prior: &[EmirRecord]) -> HashSet<&str> {
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
