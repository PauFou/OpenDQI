//! SFTR reconciliation (TR auth.083) checks. Mirror of
//! `dq::reconciliation` for the SFTR regime.

use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, ReconciliationRecord, Severity, SftrRecord};

/// A reconciliation (TR auth.083) SFTR check.
pub trait SftrReconciliationCheck: Send + Sync {
    /// Stable identifier, e.g. `SFTR.REC.UNPAIRED_TRADE`.
    fn id(&self) -> &'static str;
    /// The DQ dimension this check belongs to.
    fn dimension(&self) -> DqDimension;
    /// Default severity for issues raised by this check.
    fn severity(&self) -> Severity;
    /// Execute the check against reconciliation records + prior
    /// SFTR records.
    fn run(
        &self,
        records: &[ReconciliationRecord],
        prior: &[SftrRecord],
        ctx: &CheckContext,
    ) -> Vec<DqIssue>;
}

mod field_mismatch;
mod unpaired_trade;
mod unreconciled_trade;

pub use field_mismatch::SftrFieldMismatch;
pub use unpaired_trade::SftrUnpairedTrade;
pub use unreconciled_trade::SftrUnreconciledTrade;
