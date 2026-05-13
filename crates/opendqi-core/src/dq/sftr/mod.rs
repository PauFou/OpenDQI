//! SFTR (Securities Financing Transactions Regulation) data-quality
//! checks. Parallel to the EMIR checks living one level up.

use super::CheckContext;
use crate::model::{DqDimension, DqIssue, Severity, SftrRecord};

/// A SFTR data-quality check. Operates on `SftrRecord` slices.
pub trait SftrCheck: Send + Sync {
    /// Stable identifier, e.g. `SFTR.COMP.UTI_MISSING`.
    fn id(&self) -> &'static str;
    /// The DQ dimension this check belongs to.
    fn dimension(&self) -> DqDimension;
    /// Default severity for issues raised by this check.
    fn severity(&self) -> Severity;
    /// Execute the check against the given SFTR records.
    fn run(&self, records: &[SftrRecord], ctx: &CheckContext) -> Vec<DqIssue>;
}

mod collateral_value_missing;
mod duplicate_uti;
mod haircut_missing;
mod late_reporting;
mod missing_uti;

pub use collateral_value_missing::SftrCollateralValueMissing;
pub use duplicate_uti::SftrDuplicateUti;
pub use haircut_missing::SftrHaircutMissing;
pub use late_reporting::SftrLateReporting;
pub use missing_uti::SftrMissingUti;
