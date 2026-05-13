//! SFTR lifecycle (cross-batch) checks. Mirror the EMIR lifecycle
//! module: each check sees both the current batch and prior records
//! loaded from the history store for the UTIs in the current batch.

use std::collections::HashMap;

use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Severity, SftrRecord};

/// A lifecycle (cross-batch) SFTR check.
pub trait SftrLifecycleCheck: Send + Sync {
    /// Stable identifier, e.g. `SFTR.LFC.MODI_WITHOUT_NEWT`.
    fn id(&self) -> &'static str;
    /// The DQ dimension this check belongs to.
    fn dimension(&self) -> DqDimension;
    /// Default severity for issues raised by this check.
    fn severity(&self) -> Severity;
    /// Execute the check against the current batch + prior records.
    fn run(&self, current: &[SftrRecord], prior: &[SftrRecord], ctx: &CheckContext)
        -> Vec<DqIssue>;
}

mod duplicate_newt_for_uti;
mod etrm_without_newt;
mod modi_without_newt;

pub use duplicate_newt_for_uti::SftrDuplicateNewtForUti;
pub use etrm_without_newt::SftrEtrmWithoutNewt;
pub use modi_without_newt::SftrModiWithoutNewt;

pub(crate) fn index_priors_with_action<'a>(
    prior: &'a [SftrRecord],
    action: &str,
) -> HashMap<&'a str, Vec<&'a SftrRecord>> {
    let mut out: HashMap<&'a str, Vec<&'a SftrRecord>> = HashMap::new();
    for r in prior {
        if let (Some(uti), Some(act)) = (r.uti.as_deref(), r.action_type.as_deref()) {
            if act.eq_ignore_ascii_case(action) {
                out.entry(uti.trim()).or_default().push(r);
            }
        }
    }
    out
}
