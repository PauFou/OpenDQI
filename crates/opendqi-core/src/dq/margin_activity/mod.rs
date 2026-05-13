//! EMIR Margin Activity Report (MAR / `auth.108`) checks. Activity-
//! oriented over the history of margin postings and collections.

use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, MarginActivityRecord, Severity};

/// A MAR check.
pub trait MarginActivityCheck: Send + Sync {
    /// Stable identifier, e.g. `EMIR.MAR.POSTED_NEGATIVE`.
    fn id(&self) -> &'static str;
    /// The DQ dimension this check belongs to.
    fn dimension(&self) -> DqDimension;
    /// Default severity for issues raised by this check.
    fn severity(&self) -> Severity;
    /// Execute the check.
    fn run(
        &self,
        records: &[MarginActivityRecord],
        prior: &[MarginActivityRecord],
        ctx: &CheckContext,
    ) -> Vec<DqIssue>;
}

mod collected_negative;
mod duplicate_margin_call;
mod large_margin_delta;
mod margin_needs_currency;
mod margin_type_enum;
mod portfolio_code_missing;
mod posted_negative;
mod timeliness;

pub use collected_negative::EmirMarCollectedNegative;
pub use duplicate_margin_call::EmirMarDuplicateMarginCall;
pub use large_margin_delta::EmirMarLargeMarginDelta;
pub use margin_needs_currency::EmirMarMarginNeedsCurrency;
pub use margin_type_enum::EmirMarMarginTypeEnum;
pub use portfolio_code_missing::EmirMarPortfolioCodeMissing;
pub use posted_negative::EmirMarPostedNegative;
pub use timeliness::EmirMarTimeliness;
