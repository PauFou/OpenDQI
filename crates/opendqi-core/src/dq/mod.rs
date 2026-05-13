//! Data-quality check trait and the MVP EMIR check registry.

use chrono::{DateTime, NaiveDate, Utc};

use crate::config::Thresholds;
use crate::model::{
    DqDimension, DqIssue, EmirRecord, FeedbackRecord, MarginActivityRecord, MarginStateRecord,
    ReconciliationRecord, Severity, SftrRecord, SftrTrStateRecord, TrStateRecord,
};

mod abnormal_maturity;
mod action_event_compatibility;
mod action_type_enum;
mod asset_class_enum;
mod asset_class_missing;
mod cleared_requires_ccp;
mod clearing_status_enum;
mod clearing_status_missing;
mod collateral_portfolio_required_for_full;
mod collateralisation_category_enum;
mod commodity_base_enum;
mod commodity_requires_product_id;
mod counterparty_1_missing;
mod counterparty_2_missing;
mod cr_requires_underlying;
mod credit_sector_enum;
mod currency_notional;
mod currency_valuation;
mod duplicate_uti;
mod effective_after_maturity;
mod eq_requires_underlying;
mod etrm_requires_termination_date;
mod etrm_requires_valuation;
mod event_before_execution;
mod event_type_enum;
mod formats;
mod fx_requires_leg2_currency;
mod hedging_requires_nfc;
mod im_needs_collateral_portfolio;
mod initial_margin_missing_for_full;
mod intragroup_indicator_missing;
mod ir_requires_leg1_freq;
mod ir_requires_notional;
mod isda_version_plausible;
mod late_reporting;
mod leg1_leg2_same_currency;
mod leg2_notional_needs_currency;
mod lei_format_ccp;
mod lei_format_err;
mod lei_format_oc;
mod lei_format_rc;
mod margin_precision;
mod maru_requires_margin;
mod maru_requires_portfolio;
mod master_agreement_type_enum;
mod master_agreement_type_missing;
mod master_agreement_version_format;
mod master_agreement_version_missing;
mod maturity_in_past;
mod missing_uti;
mod missing_valuation;
mod modi_preserves_uti;
mod mtm_change_requires_valuation;
mod nature_enum;
mod nature_missing;
mod nclr_forbids_ccp;
mod negative_initial_margin_collected;
mod negative_initial_margin_posted;
mod negative_notional;
mod negative_variation_margin_collected;
mod negative_variation_margin_posted;
mod newt_forbids_prior_uti;
mod newt_forbids_termination_date;
mod notional_abnormal_magnitude;
mod notional_currency_missing;
mod notional_precision;
mod notional_precision_by_currency;
mod notional_val_currency_mismatch;
mod posc_requires_portfolio;
mod price_precision;
mod price_precision_by_currency;
mod price_requires_currency;
mod price_val_currency_mismatch;
mod product_id_missing;
mod reporting_before_execution;
mod risk_mitigation;
mod self_dealing;
mod termination_after_maturity;
mod trading_capacity_enum;
mod trading_capacity_missing;
mod valu_requires_valuation;
mod valuation_after_reporting;
mod valuation_after_termination;
mod valuation_currency_missing;
mod valuation_precision;
mod valuation_precision_by_currency;
mod valuation_timestamp_missing;
mod valuation_type_enum;
mod variation_margin_missing_for_full;
mod vm_needs_collateral_portfolio;
mod zero_notional;

pub use abnormal_maturity::AbnormalMaturity;
pub use action_event_compatibility::ActionEventCompatibility;
pub use action_type_enum::ActionTypeEnum;
pub use asset_class_enum::AssetClassEnum;
pub use asset_class_missing::AssetClassMissing;
pub use cleared_requires_ccp::ClearedRequiresCcp;
pub use clearing_status_enum::ClearingStatusEnum;
pub use clearing_status_missing::ClearingStatusMissing;
pub use collateral_portfolio_required_for_full::CollateralPortfolioRequiredForFull;
pub use collateralisation_category_enum::CollateralisationCategoryEnum;
pub use commodity_base_enum::CommodityBaseEnum;
pub use commodity_requires_product_id::CommodityRequiresProductId;
pub use counterparty_1_missing::Counterparty1Missing;
pub use counterparty_2_missing::Counterparty2Missing;
pub use cr_requires_underlying::CrRequiresUnderlying;
pub use credit_sector_enum::CreditSectorEnum;
pub use currency_notional::CurrencyNotional;
pub use currency_valuation::CurrencyValuation;
pub use duplicate_uti::DuplicateUti;
pub use effective_after_maturity::EffectiveAfterMaturity;
pub use eq_requires_underlying::EqRequiresUnderlying;
pub use etrm_requires_termination_date::EtrmRequiresTerminationDate;
pub use etrm_requires_valuation::EtrmRequiresValuation;
pub use event_before_execution::EventBeforeExecution;
pub use event_type_enum::EventTypeEnum;
pub use fx_requires_leg2_currency::FxRequiresLeg2Currency;
pub use hedging_requires_nfc::HedgingRequiresNfc;
pub use im_needs_collateral_portfolio::ImNeedsCollateralPortfolio;
pub use initial_margin_missing_for_full::InitialMarginMissingForFull;
pub use intragroup_indicator_missing::IntragroupIndicatorMissing;
pub use ir_requires_leg1_freq::IrRequiresLeg1Freq;
pub use ir_requires_notional::IrRequiresNotional;
pub use isda_version_plausible::IsdaVersionPlausible;
pub use late_reporting::LateReporting;
pub use leg1_leg2_same_currency::Leg1Leg2SameCurrency;
pub use leg2_notional_needs_currency::Leg2NotionalNeedsCurrency;
pub use lei_format_ccp::LeiFormatCcp;
pub use lei_format_err::LeiFormatErr;
pub use lei_format_oc::LeiFormatOc;
pub use lei_format_rc::LeiFormatRc;
pub use margin_precision::MarginPrecision;
pub use maru_requires_margin::MaruRequiresMargin;
pub use maru_requires_portfolio::MaruRequiresPortfolio;
pub use master_agreement_type_enum::MasterAgreementTypeEnum;
pub use master_agreement_type_missing::MasterAgreementTypeMissing;
pub use master_agreement_version_format::MasterAgreementVersionFormat;
pub use master_agreement_version_missing::MasterAgreementVersionMissing;
pub use maturity_in_past::MaturityInPast;
pub use missing_uti::MissingUti;
pub use missing_valuation::MissingValuation;
pub use modi_preserves_uti::ModiPreservesUti;
pub use mtm_change_requires_valuation::MtmChangeRequiresValuation;
pub use nature_enum::NatureEnum;
pub use nature_missing::NatureMissing;
pub use nclr_forbids_ccp::NclrForbidsCcp;
pub use negative_initial_margin_collected::NegativeInitialMarginCollected;
pub use negative_initial_margin_posted::NegativeInitialMarginPosted;
pub use negative_notional::NegativeNotional;
pub use negative_variation_margin_collected::NegativeVariationMarginCollected;
pub use negative_variation_margin_posted::NegativeVariationMarginPosted;
pub use newt_forbids_prior_uti::NewtForbidsPriorUti;
pub use newt_forbids_termination_date::NewtForbidsTerminationDate;
pub use notional_abnormal_magnitude::NotionalAbnormalMagnitude;
pub use notional_currency_missing::NotionalCurrencyMissing;
pub use notional_precision::NotionalPrecision;
pub use notional_precision_by_currency::NotionalPrecisionByCurrency;
pub use notional_val_currency_mismatch::NotionalValCurrencyMismatch;
pub use posc_requires_portfolio::PoscRequiresPortfolio;
pub use price_precision::PricePrecision;
pub use price_precision_by_currency::PricePrecisionByCurrency;
pub use price_requires_currency::PriceRequiresCurrency;
pub use price_val_currency_mismatch::PriceValCurrencyMismatch;
pub use product_id_missing::ProductIdMissing;
pub use reporting_before_execution::ReportingBeforeExecution;
pub use risk_mitigation::{
    is_uncleared, EmirRmtCollateralCategoryRequired, EmirRmtDailyValuationMissing,
    EmirRmtInitialMarginThreshold, EmirRmtIntragroupNeedsIndicator, EmirRmtLateConfirmation,
    EmirRmtMasterAgreementRequired, EmirRmtNfcAboveClearingThreshold,
    EmirRmtPortfolioReconciliationMissing, EmirRmtUnclearedNeedsConfirmation,
    EmirRmtVariationMarginMissing,
};
pub use self_dealing::SelfDealing;
pub use termination_after_maturity::TerminationAfterMaturity;
pub use trading_capacity_enum::TradingCapacityEnum;
pub use trading_capacity_missing::TradingCapacityMissing;
pub use valu_requires_valuation::ValuRequiresValuation;
pub use valuation_after_reporting::ValuationAfterReporting;
pub use valuation_after_termination::ValuationAfterTermination;
pub use valuation_currency_missing::ValuationCurrencyMissing;
pub use valuation_precision::ValuationPrecision;
pub use valuation_precision_by_currency::ValuationPrecisionByCurrency;
pub use valuation_timestamp_missing::ValuationTimestampMissing;
pub use valuation_type_enum::ValuationTypeEnum;
pub use variation_margin_missing_for_full::VariationMarginMissingForFull;
pub use vm_needs_collateral_portfolio::VmNeedsCollateralPortfolio;
pub use zero_notional::ZeroNotional;

/// Read-only context passed to every check.
///
/// Injecting `today` / `now` keeps checks deterministic: tests can pin
/// a specific date and observe identical issue output across runs.
#[derive(Debug, Clone)]
pub struct CheckContext {
    /// Threshold configuration.
    pub thresholds: Thresholds,
    /// Reference calendar date (UTC).
    pub today: NaiveDate,
    /// Reference instant.
    pub now: DateTime<Utc>,
}

impl CheckContext {
    /// Build a context using the system clock and default thresholds.
    pub fn now_with_defaults() -> Self {
        let now = Utc::now();
        Self {
            thresholds: Thresholds::default(),
            today: now.date_naive(),
            now,
        }
    }
}

/// A data-quality check. Implementations are pure functions of the
/// input records and the context.
pub trait Check: Send + Sync {
    /// Stable identifier, e.g. `EMIR.COMP.UTI_MISSING`.
    fn id(&self) -> &'static str;
    /// The DQ dimension this check belongs to.
    fn dimension(&self) -> DqDimension;
    /// Default severity for issues raised by this check.
    fn severity(&self) -> Severity;
    /// Execute the check against the given records.
    fn run(&self, records: &[EmirRecord], ctx: &CheckContext) -> Vec<DqIssue>;
}

/// The default EMIR check registry. Returned in a stable order so
/// that issue lists remain reproducible across runs.
///
/// Currently 21 checks covering all six DQ dimensions and 16 of them
/// aligned with ESMA EMIR Refit Validation Rules (`EMIR-VR-*`). See
/// `docs/emir-checks.md` for the full catalog.
pub fn default_checks() -> Vec<Box<dyn Check>> {
    vec![
        // Completeness
        Box::new(MissingUti),
        Box::new(MissingValuation),
        Box::new(Counterparty1Missing),
        Box::new(Counterparty2Missing),
        Box::new(NotionalCurrencyMissing),
        Box::new(ValuationCurrencyMissing),
        Box::new(ValuationTimestampMissing),
        Box::new(ClearingStatusMissing),
        Box::new(IntragroupIndicatorMissing),
        Box::new(NatureMissing),
        Box::new(TradingCapacityMissing),
        Box::new(MasterAgreementTypeMissing),
        Box::new(AssetClassMissing),
        Box::new(InitialMarginMissingForFull),
        Box::new(VariationMarginMissingForFull),
        Box::new(CollateralPortfolioRequiredForFull),
        // Validity
        Box::new(LeiFormatRc),
        Box::new(LeiFormatOc),
        Box::new(LeiFormatErr),
        Box::new(LeiFormatCcp),
        Box::new(CurrencyNotional),
        Box::new(CurrencyValuation),
        Box::new(ActionTypeEnum),
        Box::new(EventTypeEnum),
        Box::new(ValuationTypeEnum),
        Box::new(TradingCapacityEnum),
        Box::new(AssetClassEnum),
        Box::new(NatureEnum),
        Box::new(MasterAgreementTypeEnum),
        Box::new(CollateralisationCategoryEnum),
        Box::new(ClearingStatusEnum),
        // Accuracy
        Box::new(AbnormalMaturity),
        Box::new(ZeroNotional),
        Box::new(NegativeNotional),
        Box::new(NegativeInitialMarginPosted),
        Box::new(NegativeInitialMarginCollected),
        Box::new(NegativeVariationMarginPosted),
        Box::new(NegativeVariationMarginCollected),
        // Uniqueness
        Box::new(DuplicateUti),
        // Timeliness
        Box::new(LateReporting),
        Box::new(ValuationAfterReporting),
        // Consistency
        Box::new(ReportingBeforeExecution),
        Box::new(ClearedRequiresCcp),
        Box::new(ValuationAfterTermination),
        Box::new(NclrForbidsCcp),
        Box::new(MaruRequiresMargin),
        Box::new(EventBeforeExecution),
        Box::new(MaturityInPast),
        Box::new(TerminationAfterMaturity),
        Box::new(EffectiveAfterMaturity),
        Box::new(EtrmRequiresTerminationDate),
        // ---- Tier 2 additions (30) ----
        // Completeness
        Box::new(ProductIdMissing),
        Box::new(MasterAgreementVersionMissing),
        // Validity
        Box::new(NotionalPrecision),
        Box::new(ValuationPrecision),
        Box::new(PricePrecision),
        Box::new(MarginPrecision),
        Box::new(MasterAgreementVersionFormat),
        Box::new(IsdaVersionPlausible),
        // Accuracy
        Box::new(IrRequiresNotional),
        Box::new(FxRequiresLeg2Currency),
        Box::new(EqRequiresUnderlying),
        Box::new(CrRequiresUnderlying),
        Box::new(CommodityRequiresProductId),
        Box::new(IrRequiresLeg1Freq),
        // Consistency
        Box::new(NotionalValCurrencyMismatch),
        Box::new(PriceValCurrencyMismatch),
        Box::new(Leg1Leg2SameCurrency),
        Box::new(SelfDealing),
        Box::new(PriceRequiresCurrency),
        Box::new(ImNeedsCollateralPortfolio),
        Box::new(VmNeedsCollateralPortfolio),
        Box::new(Leg2NotionalNeedsCurrency),
        Box::new(HedgingRequiresNfc),
        Box::new(MtmChangeRequiresValuation),
        Box::new(ValuRequiresValuation),
        Box::new(NewtForbidsPriorUti),
        Box::new(PoscRequiresPortfolio),
        Box::new(MaruRequiresPortfolio),
        Box::new(NewtForbidsTerminationDate),
        Box::new(EtrmRequiresValuation),
        // ---- Tier 3 additions (8) ----
        // Validity (currency-aware precision)
        Box::new(NotionalPrecisionByCurrency),
        Box::new(ValuationPrecisionByCurrency),
        Box::new(PricePrecisionByCurrency),
        // Validity (asset-class deep)
        Box::new(CommodityBaseEnum),
        Box::new(CreditSectorEnum),
        // Accuracy (magnitude sanity)
        Box::new(NotionalAbnormalMagnitude),
        // Consistency (action × event matrix + UTI semantics)
        Box::new(ActionEventCompatibility),
        Box::new(ModiPreservesUti),
        // ---- Article 11 risk-mitigation for non-cleared OTC (10) ----
        Box::new(EmirRmtUnclearedNeedsConfirmation),
        Box::new(EmirRmtLateConfirmation),
        Box::new(EmirRmtPortfolioReconciliationMissing),
        Box::new(EmirRmtDailyValuationMissing),
        Box::new(EmirRmtVariationMarginMissing),
        Box::new(EmirRmtInitialMarginThreshold),
        Box::new(EmirRmtCollateralCategoryRequired),
        Box::new(EmirRmtNfcAboveClearingThreshold),
        Box::new(EmirRmtIntragroupNeedsIndicator),
        Box::new(EmirRmtMasterAgreementRequired),
    ]
}

/// Run every check in `checks` against `records` and return the
/// concatenated issues, sorted deterministically.
pub fn run_all(
    checks: &[Box<dyn Check>],
    records: &[EmirRecord],
    ctx: &CheckContext,
) -> Vec<DqIssue> {
    let mut issues: Vec<DqIssue> = checks.iter().flat_map(|c| c.run(records, ctx)).collect();
    sort_issues(&mut issues);
    issues
}

fn sort_issues(issues: &mut [DqIssue]) {
    issues.sort_by(|a, b| {
        a.check_id
            .cmp(&b.check_id)
            .then_with(|| a.source_file.cmp(&b.source_file))
            .then_with(|| a.record_id.cmp(&b.record_id))
    });
}

// ---- SFTR ----------------------------------------------------------

mod sftr;

pub use sftr::{
    SftrActionTypeEnum, SftrCheck, SftrCollNeedsCurrency, SftrCollateralCurrencyMissing,
    SftrCollateralPrecision, SftrCollateralValueMissing, SftrColuRequiresPortfolio,
    SftrCounterparty1Missing, SftrCounterparty2Missing, SftrCurrencyCollateral, SftrCurrencyLoan,
    SftrDuplicateUti, SftrEtrmRequiresTerminationDate, SftrGmraGmslaVersionPlausible,
    SftrHaircutMissing, SftrHaircutOutOfRange, SftrHaircutPrecision, SftrIsinCollateral,
    SftrLateReporting, SftrLeiFormatErr, SftrLeiFormatOc, SftrLeiFormatRc,
    SftrLendingFeeRequiresSleb, SftrLoanCollCurrencyMismatch, SftrLoanCurrencyMissing,
    SftrLoanNeedsCurrency, SftrLoanPrecision, SftrMasterAgreementVersionFormat,
    SftrMaturityBeforeEffective, SftrMissingUti, SftrNegativeCollateral, SftrNegativeLoan,
    SftrNewtForbidsPriorUti, SftrNewtForbidsTerminationDate, SftrRatePrecision,
    SftrRebateRequiresRepoOrBsb, SftrReuuRequiresReuseIndicator, SftrSelfDealing,
    SftrSettlementBeforeExecution, SftrSftTypeEnum, SftrSftTypeMissing,
};

/// Default SFTR check registry. 40 checks total covering all six DQ
/// dimensions; severity ranges from `Warning` to `Critical`. See
/// `docs/sftr-checks.md` for the full catalog.
pub fn default_sftr_checks() -> Vec<Box<dyn SftrCheck>> {
    vec![
        // Completeness
        Box::new(SftrMissingUti),
        Box::new(SftrCollateralValueMissing),
        Box::new(SftrHaircutMissing),
        Box::new(SftrCounterparty1Missing),
        Box::new(SftrCounterparty2Missing),
        Box::new(SftrLoanCurrencyMissing),
        Box::new(SftrCollateralCurrencyMissing),
        // Validity
        Box::new(SftrLeiFormatRc),
        Box::new(SftrLeiFormatOc),
        Box::new(SftrLeiFormatErr),
        Box::new(SftrCurrencyLoan),
        Box::new(SftrCurrencyCollateral),
        Box::new(SftrIsinCollateral),
        // Accuracy
        Box::new(SftrNegativeLoan),
        Box::new(SftrNegativeCollateral),
        Box::new(SftrHaircutOutOfRange),
        // Uniqueness
        Box::new(SftrDuplicateUti),
        // Timeliness
        Box::new(SftrLateReporting),
        // Consistency
        Box::new(SftrSettlementBeforeExecution),
        Box::new(SftrMaturityBeforeEffective),
        // ---- Tier 2 additions (20) ----
        // Completeness
        Box::new(SftrSftTypeMissing),
        // Validity
        Box::new(SftrSftTypeEnum),
        Box::new(SftrActionTypeEnum),
        Box::new(SftrLoanPrecision),
        Box::new(SftrCollateralPrecision),
        Box::new(SftrHaircutPrecision),
        Box::new(SftrRatePrecision),
        Box::new(SftrMasterAgreementVersionFormat),
        Box::new(SftrGmraGmslaVersionPlausible),
        // Consistency
        Box::new(SftrSelfDealing),
        Box::new(SftrLoanNeedsCurrency),
        Box::new(SftrCollNeedsCurrency),
        Box::new(SftrLoanCollCurrencyMismatch),
        Box::new(SftrRebateRequiresRepoOrBsb),
        Box::new(SftrLendingFeeRequiresSleb),
        Box::new(SftrNewtForbidsPriorUti),
        Box::new(SftrNewtForbidsTerminationDate),
        Box::new(SftrEtrmRequiresTerminationDate),
        Box::new(SftrColuRequiresPortfolio),
        Box::new(SftrReuuRequiresReuseIndicator),
    ]
}

/// Run every SFTR check in `checks` against `records` and return the
/// concatenated issues, sorted deterministically.
pub fn run_all_sftr(
    checks: &[Box<dyn SftrCheck>],
    records: &[SftrRecord],
    ctx: &CheckContext,
) -> Vec<DqIssue> {
    let mut issues: Vec<DqIssue> = checks.iter().flat_map(|c| c.run(records, ctx)).collect();
    sort_issues(&mut issues);
    issues
}

// ---- Lifecycle checks (cross-batch) -------------------------------
//
// Lifecycle checks see two slices: `current` (the batch being scanned)
// and `prior` (records ingested by earlier scans of the same UTIs,
// loaded from the SQLite history store). They are opt-in: the CLI
// only invokes them when `--store <path>` is set.

mod lifecycle;

pub use lifecycle::{
    DuplicateNewtForUti, EtrmWithoutNewt, LifecycleValuationAfterTermination, ModiWithoutNewt,
    ValuationRegression,
};

/// A lifecycle (cross-batch) EMIR check.
pub trait LifecycleCheck: Send + Sync {
    /// Stable identifier, e.g. `EMIR.LFC.MODI_WITHOUT_NEWT`.
    fn id(&self) -> &'static str;
    /// The DQ dimension this check belongs to.
    fn dimension(&self) -> DqDimension;
    /// Default severity for issues raised by this check.
    fn severity(&self) -> Severity;
    /// Execute the check against the current batch + the prior records.
    fn run(&self, current: &[EmirRecord], prior: &[EmirRecord], ctx: &CheckContext)
        -> Vec<DqIssue>;
}

/// Default EMIR lifecycle check registry (5 checks).
pub fn default_lifecycle_checks() -> Vec<Box<dyn LifecycleCheck>> {
    vec![
        Box::new(ModiWithoutNewt),
        Box::new(EtrmWithoutNewt),
        Box::new(DuplicateNewtForUti),
        Box::new(ValuationRegression),
        Box::new(LifecycleValuationAfterTermination),
    ]
}

/// Run every EMIR lifecycle check and return the concatenated, sorted
/// issues.
pub fn run_all_lifecycle(
    checks: &[Box<dyn LifecycleCheck>],
    current: &[EmirRecord],
    prior: &[EmirRecord],
    ctx: &CheckContext,
) -> Vec<DqIssue> {
    let mut issues: Vec<DqIssue> = checks
        .iter()
        .flat_map(|c| c.run(current, prior, ctx))
        .collect();
    sort_issues(&mut issues);
    issues
}

pub use sftr::lifecycle::{
    SftrDuplicateNewtForUti, SftrEtrmWithoutNewt, SftrLifecycleCheck, SftrModiWithoutNewt,
};

/// Default SFTR lifecycle check registry (3 checks).
pub fn default_sftr_lifecycle_checks() -> Vec<Box<dyn SftrLifecycleCheck>> {
    vec![
        Box::new(SftrModiWithoutNewt),
        Box::new(SftrEtrmWithoutNewt),
        Box::new(SftrDuplicateNewtForUti),
    ]
}

/// Run every SFTR lifecycle check and return the concatenated, sorted
/// issues.
pub fn run_all_sftr_lifecycle(
    checks: &[Box<dyn SftrLifecycleCheck>],
    current: &[SftrRecord],
    prior: &[SftrRecord],
    ctx: &CheckContext,
) -> Vec<DqIssue> {
    let mut issues: Vec<DqIssue> = checks
        .iter()
        .flat_map(|c| c.run(current, prior, ctx))
        .collect();
    sort_issues(&mut issues);
    issues
}

// ---- Feedback (TR → firm) checks ----------------------------------
//
// Feedback checks ingest TR-side ISO 20022 messages (auth.092 for EMIR,
// auth.080 for SFTR) and cross-reference them against the local history
// store. They are opt-in via the `opendqi {emir,sftr} feedback`
// subcommand.

mod feedback;

pub use feedback::{
    TrInaccurateReported, TrMissingButNotSent, TrMissingDespiteSubmission, TrRejectedUti,
};

/// A feedback (TR → firm) EMIR check.
pub trait FeedbackCheck: Send + Sync {
    /// Stable identifier, e.g. `EMIR.FBK.TR_REJECTED_UTI`.
    fn id(&self) -> &'static str;
    /// The DQ dimension this check belongs to.
    fn dimension(&self) -> DqDimension;
    /// Default severity for issues raised by this check.
    fn severity(&self) -> Severity;
    /// Execute the check against feedback records + previously
    /// persisted EMIR records.
    fn run(
        &self,
        feedback: &[FeedbackRecord],
        prior: &[EmirRecord],
        ctx: &CheckContext,
    ) -> Vec<DqIssue>;
}

/// Default EMIR feedback check registry (4 checks).
pub fn default_feedback_checks() -> Vec<Box<dyn FeedbackCheck>> {
    vec![
        Box::new(TrRejectedUti),
        Box::new(TrMissingButNotSent),
        Box::new(TrMissingDespiteSubmission),
        Box::new(TrInaccurateReported),
    ]
}

/// Run every EMIR feedback check and return the concatenated, sorted
/// issues.
pub fn run_all_feedback(
    checks: &[Box<dyn FeedbackCheck>],
    feedback: &[FeedbackRecord],
    prior: &[EmirRecord],
    ctx: &CheckContext,
) -> Vec<DqIssue> {
    let mut issues: Vec<DqIssue> = checks
        .iter()
        .flat_map(|c| c.run(feedback, prior, ctx))
        .collect();
    sort_issues(&mut issues);
    issues
}

pub use sftr::feedback::{
    SftrFeedbackCheck, SftrTrInaccurateReported, SftrTrMissingButNotSent,
    SftrTrMissingDespiteSubmission, SftrTrRejectedUti,
};

/// Default SFTR feedback check registry (4 checks).
pub fn default_sftr_feedback_checks() -> Vec<Box<dyn SftrFeedbackCheck>> {
    vec![
        Box::new(SftrTrRejectedUti),
        Box::new(SftrTrMissingButNotSent),
        Box::new(SftrTrMissingDespiteSubmission),
        Box::new(SftrTrInaccurateReported),
    ]
}

/// Run every SFTR feedback check and return the concatenated, sorted
/// issues.
pub fn run_all_sftr_feedback(
    checks: &[Box<dyn SftrFeedbackCheck>],
    feedback: &[FeedbackRecord],
    prior: &[SftrRecord],
    ctx: &CheckContext,
) -> Vec<DqIssue> {
    let mut issues: Vec<DqIssue> = checks
        .iter()
        .flat_map(|c| c.run(feedback, prior, ctx))
        .collect();
    sort_issues(&mut issues);
    issues
}

// ---- Reconciliation (TR auth.106 / auth.083) checks ---------------
//
// Each reconciliation check sees the records parsed from the TR's
// pairing / reconciliation report plus the relevant prior records
// loaded from the history store. The auth.106 / auth.083 messages
// are returned by the TR after matching the firm's submission with
// the counterparty's.

mod reconciliation;

pub use reconciliation::{EmirFieldMismatch, UnpairedTrade, UnreconciledTrade};

/// A reconciliation (TR auth.106) EMIR check.
pub trait ReconciliationCheck: Send + Sync {
    /// Stable identifier, e.g. `EMIR.REC.UNPAIRED_TRADE`.
    fn id(&self) -> &'static str;
    /// The DQ dimension this check belongs to.
    fn dimension(&self) -> DqDimension;
    /// Default severity for issues raised by this check.
    fn severity(&self) -> Severity;
    /// Execute the check against reconciliation records + prior
    /// EMIR records.
    fn run(
        &self,
        records: &[ReconciliationRecord],
        prior: &[EmirRecord],
        ctx: &CheckContext,
    ) -> Vec<DqIssue>;
}

/// Default EMIR reconciliation check registry (3 checks).
pub fn default_reconciliation_checks() -> Vec<Box<dyn ReconciliationCheck>> {
    vec![
        Box::new(UnpairedTrade),
        Box::new(UnreconciledTrade),
        Box::new(EmirFieldMismatch),
    ]
}

/// Run every EMIR reconciliation check and return the concatenated,
/// sorted issues.
pub fn run_all_reconciliation(
    checks: &[Box<dyn ReconciliationCheck>],
    records: &[ReconciliationRecord],
    prior: &[EmirRecord],
    ctx: &CheckContext,
) -> Vec<DqIssue> {
    let mut issues: Vec<DqIssue> = checks
        .iter()
        .flat_map(|c| c.run(records, prior, ctx))
        .collect();
    sort_issues(&mut issues);
    issues
}

pub use sftr::reconciliation::{
    SftrFieldMismatch, SftrReconciliationCheck, SftrUnpairedTrade, SftrUnreconciledTrade,
};

/// Default SFTR reconciliation check registry (3 checks).
pub fn default_sftr_reconciliation_checks() -> Vec<Box<dyn SftrReconciliationCheck>> {
    vec![
        Box::new(SftrUnpairedTrade),
        Box::new(SftrUnreconciledTrade),
        Box::new(SftrFieldMismatch),
    ]
}

/// Run every SFTR reconciliation check and return the concatenated,
/// sorted issues.
pub fn run_all_sftr_reconciliation(
    checks: &[Box<dyn SftrReconciliationCheck>],
    records: &[ReconciliationRecord],
    prior: &[SftrRecord],
    ctx: &CheckContext,
) -> Vec<DqIssue> {
    let mut issues: Vec<DqIssue> = checks
        .iter()
        .flat_map(|c| c.run(records, prior, ctx))
        .collect();
    sort_issues(&mut issues);
    issues
}

// ---- Trade State Report (TR auth.107) checks ---------------------
//
// State-oriented checks: each takes a snapshot of what the TR
// currently believes is outstanding, plus the relevant prior
// submission history loaded from the store (optional).

mod tr_state;

pub use tr_state::{
    EmirActivePastMaturity, EmirDuplicateActiveUti, EmirMissingValuationOnTsr,
    EmirOutstandingSummary, EmirPlaceholderMaturity, EmirStaleValuationOnTsr,
    EmirTsrValuationAfterTermination,
};

/// A Trade State Report (TSR) EMIR check.
pub trait TrStateCheck: Send + Sync {
    /// Stable identifier, e.g. `EMIR.TST.STALE_VALUATION`.
    fn id(&self) -> &'static str;
    /// The DQ dimension this check belongs to.
    fn dimension(&self) -> DqDimension;
    /// Default severity for issues raised by this check.
    fn severity(&self) -> Severity;
    /// Execute the check against the TSR snapshot + any prior
    /// EMIR records loaded from the history store.
    fn run(
        &self,
        records: &[TrStateRecord],
        prior: &[EmirRecord],
        ctx: &CheckContext,
    ) -> Vec<DqIssue>;
}

/// Default EMIR TSR check registry (7 checks).
pub fn default_tr_state_checks() -> Vec<Box<dyn TrStateCheck>> {
    vec![
        Box::new(EmirOutstandingSummary),
        Box::new(EmirStaleValuationOnTsr),
        Box::new(EmirMissingValuationOnTsr),
        Box::new(EmirActivePastMaturity),
        Box::new(EmirPlaceholderMaturity),
        Box::new(EmirDuplicateActiveUti),
        Box::new(EmirTsrValuationAfterTermination),
    ]
}

/// Run every EMIR TSR check and return the concatenated, sorted
/// issues.
pub fn run_all_tr_state(
    checks: &[Box<dyn TrStateCheck>],
    records: &[TrStateRecord],
    prior: &[EmirRecord],
    ctx: &CheckContext,
) -> Vec<DqIssue> {
    let mut issues: Vec<DqIssue> = checks
        .iter()
        .flat_map(|c| c.run(records, prior, ctx))
        .collect();
    sort_issues(&mut issues);
    issues
}

// ---- Trade Activity Report (TR auth.030 returns) checks ----------
//
// Activity-oriented checks: distributions, spikes, repeated
// corrections, duplicate NEWTs in batch, and TAR↔TSR coherence
// when both files are provided. See `docs/tr-activity-checks.md`.

mod tr_activity;

pub use tr_activity::{
    EmirDuplicateNewtInBatch, EmirNewtNotInTsr, EmirRepeatedCorrection, EmirSpikeModi,
    EmirSpikeTerm,
};

/// A Trade Activity Report (TAR) EMIR check.
pub trait TrActivityCheck: Send + Sync {
    /// Stable identifier, e.g. `EMIR.TRA.SPIKE_TERM`.
    fn id(&self) -> &'static str;
    /// The DQ dimension this check belongs to.
    fn dimension(&self) -> DqDimension;
    /// Default severity for issues raised by this check.
    fn severity(&self) -> Severity;
    /// Execute the check against the TAR batch, prior submission
    /// history, and the optional companion TSR for cross-layer
    /// coherence.
    fn run(
        &self,
        records: &[EmirRecord],
        prior: &[EmirRecord],
        tsr: Option<&[TrStateRecord]>,
        ctx: &CheckContext,
    ) -> Vec<DqIssue>;
}

/// Default EMIR TAR check registry (5 checks).
pub fn default_tr_activity_checks() -> Vec<Box<dyn TrActivityCheck>> {
    vec![
        Box::new(EmirRepeatedCorrection),
        Box::new(EmirSpikeTerm),
        Box::new(EmirSpikeModi),
        Box::new(EmirDuplicateNewtInBatch),
        Box::new(EmirNewtNotInTsr),
    ]
}

/// Run every EMIR TAR check and return the concatenated, sorted
/// issues.
pub fn run_all_tr_activity(
    checks: &[Box<dyn TrActivityCheck>],
    records: &[EmirRecord],
    prior: &[EmirRecord],
    tsr: Option<&[TrStateRecord]>,
    ctx: &CheckContext,
) -> Vec<DqIssue> {
    let mut issues: Vec<DqIssue> = checks
        .iter()
        .flat_map(|c| c.run(records, prior, tsr, ctx))
        .collect();
    sort_issues(&mut issues);
    issues
}

// ---- EMIR Margin Activity Report (auth.108) checks --------------

mod margin_activity;

pub use margin_activity::{
    EmirMarCollectedNegative, EmirMarDuplicateMarginCall, EmirMarLargeMarginDelta,
    EmirMarMarginNeedsCurrency, EmirMarMarginTypeEnum, EmirMarPortfolioCodeMissing,
    EmirMarPostedNegative, EmirMarTimeliness, MarginActivityCheck,
};

/// Default EMIR MAR check registry (8 checks).
pub fn default_margin_activity_checks() -> Vec<Box<dyn MarginActivityCheck>> {
    vec![
        Box::new(EmirMarMarginTypeEnum),
        Box::new(EmirMarPostedNegative),
        Box::new(EmirMarCollectedNegative),
        Box::new(EmirMarLargeMarginDelta),
        Box::new(EmirMarMarginNeedsCurrency),
        Box::new(EmirMarPortfolioCodeMissing),
        Box::new(EmirMarTimeliness),
        Box::new(EmirMarDuplicateMarginCall),
    ]
}

/// Run every EMIR MAR check.
pub fn run_all_margin_activity(
    checks: &[Box<dyn MarginActivityCheck>],
    records: &[MarginActivityRecord],
    prior: &[MarginActivityRecord],
    ctx: &CheckContext,
) -> Vec<DqIssue> {
    let mut issues: Vec<DqIssue> = checks
        .iter()
        .flat_map(|c| c.run(records, prior, ctx))
        .collect();
    sort_issues(&mut issues);
    issues
}

// ---- EMIR Margin State Report (auth.109) checks -----------------

mod margin_state;

pub use margin_state::{
    EmirMsrCollateralMarketValueNegative, EmirMsrCollateralizationCategoryEnum,
    EmirMsrHaircutOutOfRange, EmirMsrImImbalance, EmirMsrInitialMarginNegative,
    EmirMsrMarginMissingForOutstanding, EmirMsrMarginStale, EmirMsrVariationMarginNegative,
    MarginStateCheck,
};

/// Default EMIR MSR check registry (8 checks).
pub fn default_margin_state_checks() -> Vec<Box<dyn MarginStateCheck>> {
    vec![
        Box::new(EmirMsrInitialMarginNegative),
        Box::new(EmirMsrVariationMarginNegative),
        Box::new(EmirMsrCollateralMarketValueNegative),
        Box::new(EmirMsrMarginStale),
        Box::new(EmirMsrMarginMissingForOutstanding),
        Box::new(EmirMsrHaircutOutOfRange),
        Box::new(EmirMsrCollateralizationCategoryEnum),
        Box::new(EmirMsrImImbalance),
    ]
}

/// Run every EMIR MSR check.
pub fn run_all_margin_state(
    checks: &[Box<dyn MarginStateCheck>],
    records: &[MarginStateRecord],
    prior: &[EmirRecord],
    ctx: &CheckContext,
) -> Vec<DqIssue> {
    let mut issues: Vec<DqIssue> = checks
        .iter()
        .flat_map(|c| c.run(records, prior, ctx))
        .collect();
    sort_issues(&mut issues);
    issues
}

// ---- SFTR Trade State Report (auth.079) checks ------------------

pub use sftr::tr_state::{
    SftrActivePastMaturity, SftrDuplicateActiveUti, SftrHaircutOutOfRangeOnTsr,
    SftrMissingCollateralOnTsr, SftrOutstandingSummary, SftrStaleValuationOnTsr, SftrTrStateCheck,
};

/// Default SFTR TSR check registry (6 checks).
pub fn default_sftr_tr_state_checks() -> Vec<Box<dyn SftrTrStateCheck>> {
    vec![
        Box::new(SftrOutstandingSummary),
        Box::new(SftrStaleValuationOnTsr),
        Box::new(SftrMissingCollateralOnTsr),
        Box::new(SftrActivePastMaturity),
        Box::new(SftrDuplicateActiveUti),
        Box::new(SftrHaircutOutOfRangeOnTsr),
    ]
}

/// Run every SFTR TSR check.
pub fn run_all_sftr_tr_state(
    checks: &[Box<dyn SftrTrStateCheck>],
    records: &[SftrTrStateRecord],
    prior: &[SftrRecord],
    ctx: &CheckContext,
) -> Vec<DqIssue> {
    let mut issues: Vec<DqIssue> = checks
        .iter()
        .flat_map(|c| c.run(records, prior, ctx))
        .collect();
    sort_issues(&mut issues);
    issues
}

// ---- SFTR Trade Activity Report (auth.052 TR-replay) checks -----

pub use sftr::tr_activity::{
    SftrDuplicateNewtInBatch, SftrNewtNotInTsr, SftrRepeatedCorrection, SftrSpikeModi,
    SftrSpikeTerm, SftrTrActivityCheck,
};

/// Default SFTR TAR check registry (5 checks).
pub fn default_sftr_tr_activity_checks() -> Vec<Box<dyn SftrTrActivityCheck>> {
    vec![
        Box::new(SftrRepeatedCorrection),
        Box::new(SftrSpikeTerm),
        Box::new(SftrSpikeModi),
        Box::new(SftrDuplicateNewtInBatch),
        Box::new(SftrNewtNotInTsr),
    ]
}

/// Run every SFTR TAR check.
pub fn run_all_sftr_tr_activity(
    checks: &[Box<dyn SftrTrActivityCheck>],
    records: &[SftrRecord],
    prior: &[SftrRecord],
    tsr: Option<&[SftrTrStateRecord]>,
    ctx: &CheckContext,
) -> Vec<DqIssue> {
    let mut issues: Vec<DqIssue> = checks
        .iter()
        .flat_map(|c| c.run(records, prior, tsr, ctx))
        .collect();
    sort_issues(&mut issues);
    issues
}
