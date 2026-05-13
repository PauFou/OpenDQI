//! EMIR Margin State Report (MSR / `auth.109`) checks. State-
//! oriented over the TR's current view of margin postings.

use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, EmirRecord, MarginStateRecord, Severity};

/// A MSR check.
pub trait MarginStateCheck: Send + Sync {
    /// Stable identifier, e.g. `EMIR.MSR.INITIAL_MARGIN_NEGATIVE`.
    fn id(&self) -> &'static str;
    /// The DQ dimension this check belongs to.
    fn dimension(&self) -> DqDimension;
    /// Default severity for issues raised by this check.
    fn severity(&self) -> Severity;
    /// Execute the check.
    fn run(
        &self,
        records: &[MarginStateRecord],
        prior: &[EmirRecord],
        ctx: &CheckContext,
    ) -> Vec<DqIssue>;
}

mod collateral_market_value_negative;
mod collateralization_category_enum;
mod haircut_out_of_range;
mod im_imbalance;
mod initial_margin_negative;
mod margin_missing_for_outstanding;
mod margin_stale;
mod variation_margin_negative;

pub use collateral_market_value_negative::EmirMsrCollateralMarketValueNegative;
pub use collateralization_category_enum::EmirMsrCollateralizationCategoryEnum;
pub use haircut_out_of_range::EmirMsrHaircutOutOfRange;
pub use im_imbalance::EmirMsrImImbalance;
pub use initial_margin_negative::EmirMsrInitialMarginNegative;
pub use margin_missing_for_outstanding::EmirMsrMarginMissingForOutstanding;
pub use margin_stale::EmirMsrMarginStale;
pub use variation_margin_negative::EmirMsrVariationMarginNegative;
