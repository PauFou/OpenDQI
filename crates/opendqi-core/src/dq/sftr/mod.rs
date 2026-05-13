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

mod collateral_currency_missing;
mod collateral_value_missing;
mod counterparty_1_missing;
mod counterparty_2_missing;
mod currency_collateral;
mod currency_loan;
mod duplicate_uti;
mod haircut_missing;
mod haircut_out_of_range;
mod isin_collateral;
mod late_reporting;
mod lei_format_err;
mod lei_format_oc;
mod lei_format_rc;
mod loan_currency_missing;
mod maturity_before_effective;
mod missing_uti;
mod negative_collateral;
mod negative_loan;
mod settlement_before_execution;

pub use collateral_currency_missing::SftrCollateralCurrencyMissing;
pub use collateral_value_missing::SftrCollateralValueMissing;
pub use counterparty_1_missing::SftrCounterparty1Missing;
pub use counterparty_2_missing::SftrCounterparty2Missing;
pub use currency_collateral::SftrCurrencyCollateral;
pub use currency_loan::SftrCurrencyLoan;
pub use duplicate_uti::SftrDuplicateUti;
pub use haircut_missing::SftrHaircutMissing;
pub use haircut_out_of_range::SftrHaircutOutOfRange;
pub use isin_collateral::SftrIsinCollateral;
pub use late_reporting::SftrLateReporting;
pub use lei_format_err::SftrLeiFormatErr;
pub use lei_format_oc::SftrLeiFormatOc;
pub use lei_format_rc::SftrLeiFormatRc;
pub use loan_currency_missing::SftrLoanCurrencyMissing;
pub use maturity_before_effective::SftrMaturityBeforeEffective;
pub use missing_uti::SftrMissingUti;
pub use negative_collateral::SftrNegativeCollateral;
pub use negative_loan::SftrNegativeLoan;
pub use settlement_before_execution::SftrSettlementBeforeExecution;
