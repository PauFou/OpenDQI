//! EMIR Derivatives Trade Position Set Report (`auth.090`)
//! granular checks — v0.18 E5. Per-record defects on
//! [`crate::EmirPositionSetRecord`].
//!
//! No `prior` argument on `run` — auth.090 is a single-snapshot
//! report and v0.18 has no position-history lifecycle tracking.
//!
//! Granular checks complement the 4 v0.18 aggregate Position
//! DQIs sourced from the same slice (E4): DQIs roll up the
//! rate, these per-record checks pinpoint the exact violating
//! row.

use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, EmirPositionSetRecord, Severity};

/// An EMIR auth.090 position-set check.
pub trait EmirPosCheck: Send + Sync {
    /// Stable identifier, e.g. `EMIR.POS.NOTIONAL_NEGATIVE`.
    fn id(&self) -> &'static str;
    /// The DQ dimension this check belongs to.
    fn dimension(&self) -> DqDimension;
    /// Default severity for issues raised by this check.
    fn severity(&self) -> Severity;
    /// Execute the check over the position-set records slice.
    fn run(&self, records: &[EmirPositionSetRecord], ctx: &CheckContext) -> Vec<DqIssue>;
}

mod asset_class_enum_invalid;
mod jurisdiction_missing;
mod notional_negative;
mod position_set_kind_invalid;

pub use asset_class_enum_invalid::EmirPosAssetClassEnumInvalid;
pub use jurisdiction_missing::EmirPosJurisdictionMissing;
pub use notional_negative::EmirPosNotionalNegative;
pub use position_set_kind_invalid::EmirPosPositionSetKindInvalid;
