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

mod action_type_enum;
mod coll_needs_currency;
mod collateral_currency_missing;
mod collateral_precision;
mod collateral_value_missing;
mod colu_requires_portfolio;
mod counterparty_1_missing;
mod counterparty_2_missing;
mod currency_collateral;
mod currency_loan;
mod duplicate_uti;
mod etrm_requires_termination_date;
mod gmra_gmsla_version_plausible;
mod haircut_missing;
mod haircut_out_of_range;
mod haircut_precision;
mod isin_collateral;
mod late_reporting;
mod lei_format_err;
mod lei_format_oc;
mod lei_format_rc;
mod lending_fee_requires_sleb;
mod loan_coll_currency_mismatch;
mod loan_currency_missing;
mod loan_needs_currency;
mod loan_precision;
mod master_agreement_version_format;
mod maturity_before_effective;
mod missing_uti;
mod negative_collateral;
mod negative_loan;
mod newt_forbids_prior_uti;
mod newt_forbids_termination_date;
mod rate_precision;
mod rebate_requires_repo_or_bsb;
mod reuu_requires_reuse_indicator;
mod self_dealing;
mod settlement_before_execution;
mod sft_type_enum;
mod sft_type_missing;

pub use action_type_enum::SftrActionTypeEnum;
pub use coll_needs_currency::SftrCollNeedsCurrency;
pub use collateral_currency_missing::SftrCollateralCurrencyMissing;
pub use collateral_precision::SftrCollateralPrecision;
pub use collateral_value_missing::SftrCollateralValueMissing;
pub use colu_requires_portfolio::SftrColuRequiresPortfolio;
pub use counterparty_1_missing::SftrCounterparty1Missing;
pub use counterparty_2_missing::SftrCounterparty2Missing;
pub use currency_collateral::SftrCurrencyCollateral;
pub use currency_loan::SftrCurrencyLoan;
pub use duplicate_uti::SftrDuplicateUti;
pub use etrm_requires_termination_date::SftrEtrmRequiresTerminationDate;
pub use gmra_gmsla_version_plausible::SftrGmraGmslaVersionPlausible;
pub use haircut_missing::SftrHaircutMissing;
pub use haircut_out_of_range::SftrHaircutOutOfRange;
pub use haircut_precision::SftrHaircutPrecision;
pub use isin_collateral::SftrIsinCollateral;
pub use late_reporting::SftrLateReporting;
pub use lei_format_err::SftrLeiFormatErr;
pub use lei_format_oc::SftrLeiFormatOc;
pub use lei_format_rc::SftrLeiFormatRc;
pub use lending_fee_requires_sleb::SftrLendingFeeRequiresSleb;
pub use loan_coll_currency_mismatch::SftrLoanCollCurrencyMismatch;
pub use loan_currency_missing::SftrLoanCurrencyMissing;
pub use loan_needs_currency::SftrLoanNeedsCurrency;
pub use loan_precision::SftrLoanPrecision;
pub use master_agreement_version_format::SftrMasterAgreementVersionFormat;
pub use maturity_before_effective::SftrMaturityBeforeEffective;
pub use missing_uti::SftrMissingUti;
pub use negative_collateral::SftrNegativeCollateral;
pub use negative_loan::SftrNegativeLoan;
pub use newt_forbids_prior_uti::SftrNewtForbidsPriorUti;
pub use newt_forbids_termination_date::SftrNewtForbidsTerminationDate;
pub use rate_precision::SftrRatePrecision;
pub use rebate_requires_repo_or_bsb::SftrRebateRequiresRepoOrBsb;
pub use reuu_requires_reuse_indicator::SftrReuuRequiresReuseIndicator;
pub use self_dealing::SftrSelfDealing;
pub use settlement_before_execution::SftrSettlementBeforeExecution;
pub use sft_type_enum::SftrSftTypeEnum;
pub use sft_type_missing::SftrSftTypeMissing;
