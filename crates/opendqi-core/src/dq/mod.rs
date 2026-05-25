//! Data-quality check trait and the MVP EMIR check registry.

use std::sync::Mutex;

use chrono::{DateTime, NaiveDate, Utc};
use rayon::prelude::*;

use crate::config::Thresholds;
use crate::model::{
    DqDimension, DqIssue, EmirRecord, FeedbackRecord, MarginActivityRecord, MarginStateRecord,
    MissingCollateralRecord, ReconStatsRecord, ReconciliationRecord, RejectionProfile, Severity,
    SftrMarginActivityRecord, SftrMarginStateRecord, SftrRecord, SftrReuseActivityRecord,
    SftrTrStateRecord, TrStateRecord, TradeWarningsRecord, WarningsCounterpartyRecord,
    WarningsTransactionRecord,
};

mod aggregate;
pub use aggregate::IssueAggregator;

mod sink;
pub use sink::{SortedIssueSink, SortedIssues};

pub mod dqi;

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
mod coverage;
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
mod margin_activity_lifecycle;
mod margin_precision;
mod margin_state_lifecycle;
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
mod sftr_tr_state_lifecycle;
mod termination_after_maturity;
mod tr_state_lifecycle;
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
pub use coverage::{
    EmirCommercialOrTreasuryRequiresNfc, EmirCorporateSectorEnum, EmirDeltaOutOfRange,
    EmirGammaNegative, EmirPriceNegative, EmirReportingObligationIndicatorEnum, SftrEventTypeEnum,
    SftrLendingFeeNegative, SftrMasterAgreementTypeEnum, SftrReuseIndicatorRequiresPortfolio,
};
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
    is_uncleared, EmirRmtCollateralCategoryRequired, EmirRmtCompressionEventIncomplete,
    EmirRmtDailyValuationMissing, EmirRmtInitialMarginThreshold, EmirRmtIntragroupNeedsIndicator,
    EmirRmtLateConfirmation, EmirRmtMasterAgreementRequired, EmirRmtNfcAboveClearingThreshold,
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
        // ---- Article 11 risk-mitigation for non-cleared OTC (11) ----
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
        Box::new(EmirRmtCompressionEventIncomplete),
        // ---- Field-coverage gap-fillers (6) ----
        Box::new(EmirCorporateSectorEnum),
        Box::new(EmirReportingObligationIndicatorEnum),
        Box::new(EmirPriceNegative),
        Box::new(EmirDeltaOutOfRange),
        Box::new(EmirGammaNegative),
        Box::new(EmirCommercialOrTreasuryRequiresNfc),
    ]
}

/// Run `run` over every check **in parallel**, append each check's
/// issues into one shared sink as it finishes, then
/// [`finalize_issues`]. The shared chokepoint behind every
/// `run_all*` entry point.
///
/// The parallel dimension is the **checks** axis (the high-leverage
/// choice — see `docs/performance.md`): `par_iter().for_each` runs
/// every `run(c)` concurrently; only the small `append` is
/// serialised (one lock acquire **per check**, ≤ ~150 total — the
/// `is_empty` guard skips the lock for the common zero-issue checks;
/// the expensive `run(c)` over N records is outside the lock).
///
/// **Why a single sink, not a `collect`:** every collect/flatten
/// strategy holds *all* per-check `Vec`s plus a destination at
/// once ⇒ ≈2× the issue data resident (dhat-proven for both the
/// `flat_map_iter` collect, M0.18, and the `Vec<Vec<_>>` collect,
/// M0.19). Appending into one growing buffer frees each per-check
/// `Vec` as its check finishes, so in-flight per-check `Vec`s are
/// bounded by the worker count, not the check count — there is no
/// ≈total transient and no second concat copy (Milestone 0.20).
///
/// Output is byte-identical: append order is irrelevant because
/// `finalize_issues` re-imposes the deterministic content
/// total-order over the single `Vec`.
fn collect_finalize<T, F>(checks: &[T], ctx: &CheckContext, run: F) -> Vec<DqIssue>
where
    T: Sync,
    F: Fn(&T) -> Vec<DqIssue> + Sync + Send,
{
    let sink: Mutex<Vec<DqIssue>> = Mutex::new(Vec::new());
    checks.par_iter().for_each(|c| {
        let mut produced = run(c);
        if produced.is_empty() {
            return;
        }
        sink.lock()
            .expect("issue sink mutex (a check panicked)")
            .append(&mut produced);
    });
    let mut issues = sink.into_inner().expect("issue sink mutex");
    finalize_issues(&mut issues, ctx);
    issues
}

/// Run every check in `checks` against `records` and return the
/// concatenated issues, sorted deterministically.
pub fn run_all(
    checks: &[Box<dyn Check>],
    records: &[EmirRecord],
    ctx: &CheckContext,
) -> Vec<DqIssue> {
    collect_finalize(checks, ctx, |c| c.run(records, ctx))
}

/// Production spill threshold (issue count) for the streaming
/// `run_scan` pipeline: small enough that the synthetic golden
/// fixtures never spill (so `issues.csv` stays *literally* the
/// `finalize_issues` order — byte-identical), large enough to bound
/// memory at scale (a 1 M-record EMIR scan keeps ≤ this many issues
/// resident plus on-disk runs, not the whole ~10 M-issue `Vec`).
pub const STREAM_SPILL_MAX_ISSUES: usize = 65_536;

/// Run a slice of checks in parallel and stream each check's issues
/// into `sink` as it finishes — no materialised union `Vec<DqIssue>`.
///
/// Regime/family-agnostic (Milestone 0.24): the `run` closure
/// captures the record(s)/prior/`ctx`, so this one helper feeds
/// **every** `run_all_*` family (`Check`, `SftrCheck`, feedback,
/// lifecycle, tr-state, margin, …) without per-regime variants.
/// Mirrors [`run_all`]'s parallel-over-checks shape but feeds the
/// spill-capable [`SortedIssueSink`] instead of returning a `Vec`:
/// the expensive `run` is outside the lock; one `push_batch` lock
/// acquire per non-empty check (a spill, if any, happens under it —
/// rare and I/O-bound). The caller pushes any other issue sources
/// into the same `sink`, then `finish()`es it.
pub fn stream_checks_into<C: ?Sized + Sync>(
    checks: &[Box<C>],
    sink: &std::sync::Mutex<SortedIssueSink>,
    run: impl Fn(&C) -> Vec<DqIssue> + Sync + Send,
) {
    checks.par_iter().for_each(|c| {
        let produced = run(&**c);
        if !produced.is_empty() {
            sink.lock()
                .expect("issue sink mutex (a check panicked)")
                .push_batch(produced);
        }
    });
}

/// EMIR single-batch convenience over [`stream_checks_into`] (the
/// Milestone 0.23 `run_scan` call site; kept byte-identical).
pub fn stream_emir_checks_into(
    checks: &[Box<dyn Check>],
    records: &[EmirRecord],
    ctx: &CheckContext,
    sink: &std::sync::Mutex<SortedIssueSink>,
) {
    stream_checks_into(checks, sink, |c| c.run(records, ctx));
}

/// Deterministic **content** total order over a `DqIssue`: by
/// `check_id`, `source_file`, `record_id` (the historical gross
/// order), then `uti`, `field`, `value`, `message`, `evidence`.
/// `regime`/`severity`/`dimension` are intentionally excluded — two
/// issues equal on the eight compared fields are byte-identical in
/// `issues.csv`, so their relative order is immaterial (exactly as
/// `sort_unstable_by` already leaves such pairs).
///
/// **Single source of truth** for `issues.csv` ordering: used by
/// both [`sort_issues`] and the streaming `SortedIssueSink` k-way
/// merge so the in-memory sort and the external merge cannot drift.
pub(crate) fn issue_cmp(a: &DqIssue, b: &DqIssue) -> std::cmp::Ordering {
    a.check_id
        .cmp(&b.check_id)
        .then_with(|| a.source_file.cmp(&b.source_file))
        .then_with(|| a.record_id.cmp(&b.record_id))
        .then_with(|| a.uti.cmp(&b.uti))
        .then_with(|| a.field.cmp(&b.field))
        .then_with(|| a.value.cmp(&b.value))
        .then_with(|| a.message.cmp(&b.message))
        .then_with(|| a.evidence.cmp(&b.evidence))
}

/// Sort issues into the deterministic content order ([`issue_cmp`]).
///
/// Uses `sort_unstable_by` deliberately: the stable sort allocates an
/// O(n) scratch buffer of `size_of::<DqIssue>()` per element (≈ the
/// 2.35 GiB observed at the EMIR 1M `finalize_issues`); the unstable
/// sort is fully in place. Because [`issue_cmp`] is a total order on
/// the compared content, dropping stability does not change output.
///
/// Public so CLI / server runners can post-process issue lists they
/// assemble themselves.
pub fn sort_issues(issues: &mut [DqIssue]) {
    issues.sort_unstable_by(issue_cmp);
}

/// Apply `thresholds.severity_overrides` in-place: any issue whose
/// `check_id` matches a key in the override map has its severity
/// replaced. A single chokepoint so a YAML configuration can
/// downgrade a noisy check from `high` to `warning` (or escalate the
/// other way) without touching the check code.
pub fn apply_severity_overrides(issues: &mut [DqIssue], thresholds: &Thresholds) {
    if thresholds.severity_overrides.is_empty() {
        return;
    }
    for issue in issues.iter_mut() {
        if let Some(sev) = thresholds.severity_overrides.get(&issue.check_id) {
            issue.severity = *sev;
        }
    }
}

/// Post-processing pipeline applied at the end of every `run_all_*`:
/// apply severity overrides, then sort issues deterministically.
pub fn finalize_issues(issues: &mut [DqIssue], ctx: &CheckContext) {
    apply_severity_overrides(issues, &ctx.thresholds);
    sort_issues(issues);
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
    SftrRebateRequiresRepoOrBsb, SftrReuuRequiresReuseIndicator, SftrSecurityIdentifierMissing,
    SftrSelfDealing, SftrSettlementBeforeExecution, SftrSftTypeEnum, SftrSftTypeMissing,
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
        // ---- Field-coverage gap-fillers (5) ----
        Box::new(SftrReuseIndicatorRequiresPortfolio),
        Box::new(SftrEventTypeEnum),
        Box::new(SftrMasterAgreementTypeEnum),
        Box::new(SftrLendingFeeNegative),
        Box::new(SftrSecurityIdentifierMissing),
        // ---- Margin lending — activity layer (5) ----
        Box::new(sftr::margin_activity::SftrMarMgldNeedsLoanValue),
        Box::new(sftr::margin_activity::SftrMarMgldNeedsCollateral),
        Box::new(sftr::margin_activity::SftrMarMaruRequiresValueOrHaircut),
        Box::new(sftr::margin_activity::SftrMarMaruRequiresPortfolio),
        Box::new(sftr::margin_activity::SftrMarMgldHaircutOutOfRange),
        // ---- Parity push vers EMIR (9) ----
        Box::new(sftr::parity::SftrAbnormalMaturity),
        Box::new(sftr::parity::SftrLoanAbnormalMagnitude),
        Box::new(sftr::parity::SftrEventBeforeExecution),
        Box::new(sftr::parity::SftrReportingBeforeExecution),
        Box::new(sftr::parity::SftrMaturityInPast),
        Box::new(sftr::parity::SftrTerminationAfterMaturity),
        Box::new(sftr::parity::SftrModiPreservesUti),
        Box::new(sftr::parity::SftrActionEventCompatibility),
        Box::new(sftr::parity::SftrLateReportingSettlement),
    ]
}

/// Run every SFTR check in `checks` against `records` and return the
/// concatenated issues, sorted deterministically.
pub fn run_all_sftr(
    checks: &[Box<dyn SftrCheck>],
    records: &[SftrRecord],
    ctx: &CheckContext,
) -> Vec<DqIssue> {
    collect_finalize(checks, ctx, |c| c.run(records, ctx))
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
    collect_finalize(checks, ctx, |c| c.run(current, prior, ctx))
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
    collect_finalize(checks, ctx, |c| c.run(current, prior, ctx))
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
    collect_finalize(checks, ctx, |c| c.run(feedback, prior, ctx))
}

// SFTR has no rejection-feedback message: real auth.080 is a
// reconciliation status advice (see `opendqi sftr reconcile`). The
// synthetic SFTR feedback parser and the `SFTR.FBK.*` checks were
// removed in Milestone 0.4 — there is no SFTR feedback registry.

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
    collect_finalize(checks, ctx, |c| c.run(records, prior, ctx))
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
    collect_finalize(checks, ctx, |c| c.run(records, prior, ctx))
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
    collect_finalize(checks, ctx, |c| c.run(records, prior, ctx))
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
    collect_finalize(checks, ctx, |c| c.run(records, prior, tsr, ctx))
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
    collect_finalize(checks, ctx, |c| c.run(records, prior, ctx))
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
    collect_finalize(checks, ctx, |c| c.run(records, prior, ctx))
}

// ---- SFTR Margin Data Transaction State Report (auth.085) checks ----

mod sftr_msr;

pub use sftr_msr::{
    SftrMsrCheck, SftrT3ImPostedMissing, SftrT3ImReceivedMissing, SftrT3MarginCurrencyMissing,
    SftrT3MarginNegative, SftrT3VmPostedMissing, SftrT3VmReceivedMissing,
};

/// Default SFTR MSR check registry (6 SFTR.T3.* checks).
pub fn default_sftr_msr_checks() -> Vec<Box<dyn SftrMsrCheck>> {
    vec![
        Box::new(SftrT3ImPostedMissing),
        Box::new(SftrT3VmPostedMissing),
        Box::new(SftrT3ImReceivedMissing),
        Box::new(SftrT3VmReceivedMissing),
        Box::new(SftrT3MarginNegative),
        Box::new(SftrT3MarginCurrencyMissing),
    ]
}

/// Run every SFTR MSR check over the provided records slice.
/// No `prior` argument — auth.085 is portfolio-level and v0.17
/// has no MSR lifecycle tracking.
pub fn run_all_sftr_msr(
    checks: &[Box<dyn SftrMsrCheck>],
    records: &[SftrMarginStateRecord],
    ctx: &CheckContext,
) -> Vec<DqIssue> {
    collect_finalize(checks, ctx, |c| c.run(records, ctx))
}

// ---- SFTR Margin Data Transaction Report (auth.070) checks ----
// v0.18 A5. Event-driven mirror of the auth.085 sftr_msr module.

mod sftr_mar;

pub use sftr_mar::{
    SftrMarActionTypeEnumInvalid, SftrMarAmountCurrencyMissing, SftrMarCheck,
    SftrMarEventDateInFuture, SftrMarEventWithoutPortfolio,
};

/// Default SFTR MAR check registry (4 SFTR.MAR.* checks).
///
/// No `prior` argument on the per-check `run` — v0.18 has no
/// cross-batch MAR-history tracking (amount-change implausibility
/// deferred to v0.19+).
pub fn default_sftr_mar_checks() -> Vec<Box<dyn SftrMarCheck>> {
    vec![
        Box::new(SftrMarActionTypeEnumInvalid),
        Box::new(SftrMarEventWithoutPortfolio),
        Box::new(SftrMarEventDateInFuture),
        Box::new(SftrMarAmountCurrencyMissing),
    ]
}

/// Run every SFTR MAR check over the provided records slice.
pub fn run_all_sftr_mar(
    checks: &[Box<dyn SftrMarCheck>],
    records: &[SftrMarginActivityRecord],
    ctx: &CheckContext,
) -> Vec<DqIssue> {
    collect_finalize(checks, ctx, |c| c.run(records, ctx))
}

// ---- SFTR Reused Collateral Data Report (auth.071) checks ----
// v0.18 B5. Per-record granular defects on the reuse activity layer.

mod sftr_reu;

pub use sftr_reu::{SftrReuCheck, SftrReuMissingReuseCurrency, SftrReuRateOutsidePlausibleBand};

/// Default SFTR REU check registry (2 SFTR.REU.* checks).
pub fn default_sftr_reu_checks() -> Vec<Box<dyn SftrReuCheck>> {
    vec![
        Box::new(SftrReuMissingReuseCurrency),
        Box::new(SftrReuRateOutsidePlausibleBand),
    ]
}

/// Run every SFTR REU check over the provided records slice.
pub fn run_all_sftr_reu(
    checks: &[Box<dyn SftrReuCheck>],
    records: &[SftrReuseActivityRecord],
    ctx: &CheckContext,
) -> Vec<DqIssue> {
    collect_finalize(checks, ctx, |c| c.run(records, ctx))
}

// ---- SFTR Trade State Report (auth.079) checks ------------------

pub use sftr::tr_state::{
    SftrActivePastMaturity, SftrDuplicateActiveUti, SftrHaircutOutOfRangeOnTsr,
    SftrMissingCollateralOnTsr, SftrOutstandingSummary, SftrStaleValuationOnTsr, SftrTrStateCheck,
};

/// Default SFTR TSR check registry (12 checks — 6 state-health + 6
/// MGLD-specific margin-lending state-oriented checks).
pub fn default_sftr_tr_state_checks() -> Vec<Box<dyn SftrTrStateCheck>> {
    vec![
        Box::new(SftrOutstandingSummary),
        Box::new(SftrStaleValuationOnTsr),
        Box::new(SftrMissingCollateralOnTsr),
        Box::new(SftrActivePastMaturity),
        Box::new(SftrDuplicateActiveUti),
        Box::new(SftrHaircutOutOfRangeOnTsr),
        // ---- Margin lending — state layer (6) ----
        Box::new(sftr::margin_state::SftrMsrMgldOutstandingNeedsLoan),
        Box::new(sftr::margin_state::SftrMsrMgldHaircutOutOfRange),
        Box::new(sftr::margin_state::SftrMsrMgldCollateralUnderLoan),
        Box::new(sftr::margin_state::SftrMsrMgldReuseRequiresPortfolio),
        Box::new(sftr::margin_state::SftrMsrMgldLoanCollCurrencyMismatch),
        Box::new(sftr::margin_state::SftrMsrMgldMissingIsin),
    ]
}

/// Run every SFTR TSR check.
pub fn run_all_sftr_tr_state(
    checks: &[Box<dyn SftrTrStateCheck>],
    records: &[SftrTrStateRecord],
    prior: &[SftrRecord],
    ctx: &CheckContext,
) -> Vec<DqIssue> {
    collect_finalize(checks, ctx, |c| c.run(records, prior, ctx))
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
    collect_finalize(checks, ctx, |c| c.run(records, prior, tsr, ctx))
}

// ---- Cross-batch lifecycle layers (Phase 7) ---------------------

pub use margin_activity_lifecycle::{
    EmirMarLfcNegativeTrend, EmirMarLfcPortfolioGap, EmirMarLfcRecurringLateMargin,
};
pub use margin_state_lifecycle::{
    EmirMsrLfcCategoryChanged, EmirMsrLfcCollateralMarketValueRegression, EmirMsrLfcHaircutDrift,
    MarginStateLifecycleCheck,
};
pub use sftr_tr_state_lifecycle::{
    SftrTrStateLifecycleCheck, SftrTstLfcCollateralValueRegression, SftrTstLfcHaircutChanged,
    SftrTstLfcUtiDroppedWithoutTermination,
};
pub use tr_state_lifecycle::{
    EmirTstLfcCollateralPortfolioChanged, EmirTstLfcMaturityChanged,
    EmirTstLfcUtiDroppedWithoutTermination, EmirTstLfcValuationRegression, TrStateLifecycleCheck,
};

/// Default EMIR TSR lifecycle check registry (4 checks).
pub fn default_tr_state_lifecycle_checks() -> Vec<Box<dyn TrStateLifecycleCheck>> {
    vec![
        Box::new(EmirTstLfcUtiDroppedWithoutTermination),
        Box::new(EmirTstLfcValuationRegression),
        Box::new(EmirTstLfcMaturityChanged),
        Box::new(EmirTstLfcCollateralPortfolioChanged),
    ]
}

/// Run every EMIR TSR lifecycle check.
pub fn run_all_tr_state_lifecycle(
    checks: &[Box<dyn TrStateLifecycleCheck>],
    current: &[TrStateRecord],
    prior: &[TrStateRecord],
    ctx: &CheckContext,
) -> Vec<DqIssue> {
    collect_finalize(checks, ctx, |c| c.run(current, prior, ctx))
}

/// Default SFTR TSR lifecycle check registry (3 checks).
pub fn default_sftr_tr_state_lifecycle_checks() -> Vec<Box<dyn SftrTrStateLifecycleCheck>> {
    vec![
        Box::new(SftrTstLfcUtiDroppedWithoutTermination),
        Box::new(SftrTstLfcCollateralValueRegression),
        Box::new(SftrTstLfcHaircutChanged),
    ]
}

/// Run every SFTR TSR lifecycle check.
pub fn run_all_sftr_tr_state_lifecycle(
    checks: &[Box<dyn SftrTrStateLifecycleCheck>],
    current: &[SftrTrStateRecord],
    prior: &[SftrTrStateRecord],
    ctx: &CheckContext,
) -> Vec<DqIssue> {
    collect_finalize(checks, ctx, |c| c.run(current, prior, ctx))
}

/// Default EMIR MAR cross-batch check registry (3 checks). Re-uses
/// the standard `MarginActivityCheck` trait — runner just passes the
/// prior batch loaded from the store.
pub fn default_margin_activity_lifecycle_checks() -> Vec<Box<dyn MarginActivityCheck>> {
    vec![
        Box::new(EmirMarLfcPortfolioGap),
        Box::new(EmirMarLfcRecurringLateMargin),
        Box::new(EmirMarLfcNegativeTrend),
    ]
}

/// Default EMIR MSR lifecycle check registry (3 checks).
pub fn default_margin_state_lifecycle_checks() -> Vec<Box<dyn MarginStateLifecycleCheck>> {
    vec![
        Box::new(EmirMsrLfcCollateralMarketValueRegression),
        Box::new(EmirMsrLfcCategoryChanged),
        Box::new(EmirMsrLfcHaircutDrift),
    ]
}

/// Run every EMIR MSR lifecycle check.
pub fn run_all_margin_state_lifecycle(
    checks: &[Box<dyn MarginStateLifecycleCheck>],
    current: &[MarginStateRecord],
    prior: &[MarginStateRecord],
    ctx: &CheckContext,
) -> Vec<DqIssue> {
    collect_finalize(checks, ctx, |c| c.run(current, prior, ctx))
}

// ---- EMIR pre-submission checks (rejection-profile driven) -----
//
// Closes the loop: post-TR analytics → rejection_profile.yml →
// pre-submission scan. Opt-in via `opendqi emir scan --rejection-
// profile <path>`.

mod pre_submission;

pub use pre_submission::{
    default_pre_submission_checks, EmirPscLikelyRejectionPattern, EmirPscRepeatedRejection,
    PreSubmissionCheck,
};

/// Run every EMIR pre-submission check against the records + profile.
pub fn run_all_pre_submission(
    checks: &[Box<dyn PreSubmissionCheck>],
    records: &[EmirRecord],
    profile: &RejectionProfile,
    ctx: &CheckContext,
) -> Vec<DqIssue> {
    collect_finalize(checks, ctx, |c| c.run(records, profile, ctx))
}

// ---- Book vs TSR reconciliation (shared CLI + web UI) ----------

mod book_reconcile;
mod collateral_audit;
mod tr_audit;

pub use book_reconcile::{
    compute_book_reconcile_issues, compute_sftr_book_reconcile_issues, valuation_within_tolerance,
    value_within_tolerance, BOOK_VALUATION_TOLERANCE_PCT, SFTR_BOOK_VALUE_TOLERANCE_PCT,
};
pub use collateral_audit::compute_collateral_emir_issues;
pub use tr_audit::{compute_tr_audit_emir_issues, compute_tr_audit_sftr_issues};

// ---- SFTR pre-submission checks (rejection-profile driven) ------

pub use sftr::pre_submission::{
    default_sftr_pre_submission_checks, SftrPreSubmissionCheck, SftrPscLikelyRejectionPattern,
    SftrPscRepeatedRejection,
};

/// Run every SFTR pre-submission check against the records + profile.
pub fn run_all_sftr_pre_submission(
    checks: &[Box<dyn SftrPreSubmissionCheck>],
    records: &[SftrRecord],
    profile: &RejectionProfile,
    ctx: &CheckContext,
) -> Vec<DqIssue> {
    collect_finalize(checks, ctx, |c| c.run(records, profile, ctx))
}

// ---- EMIR Reconciliation Statistics (auth.091) checks ----------

mod emir_recon_stats;

pub use emir_recon_stats::{
    default_recon_stats_checks, EmirRstOutstandingUnpairedHigh, EmirRstPairingRateLow,
    EmirRstPairingRateTrendDown, EmirRstReconRateLow, ReconStatsCheck,
};

/// Run every EMIR auth.091 check.
pub fn run_all_recon_stats(
    checks: &[Box<dyn ReconStatsCheck>],
    records: &[ReconStatsRecord],
    prior: &[ReconStatsRecord],
    ctx: &CheckContext,
) -> Vec<DqIssue> {
    collect_finalize(checks, ctx, |c| c.run(records, prior, ctx))
}

// ---- EMIR Data-Quality Warnings (auth.106) checks --------------

mod emir_warnings;

pub use emir_warnings::{
    default_warnings_checks, default_warnings_counterparty_checks,
    default_warnings_transaction_checks, EmirWrnAbnormalValuesHigh,
    EmirWrnCtrptyAbnormalValuesHigh, EmirWrnCtrptyMissingMarginInfoHigh,
    EmirWrnCtrptyMissingValuationHigh, EmirWrnCtrptyOutdatedMarginInfoHigh,
    EmirWrnCtrptyOutdatedValuationHigh, EmirWrnMissingMarginInfoHigh, EmirWrnMissingValuationHigh,
    EmirWrnOutdatedMarginInfoHigh, EmirWrnOutdatedValuationHigh, EmirWrnTxAbnormalValue,
    EmirWrnTxMissingMargin, EmirWrnTxMissingValuation, WarningsCheck, WarningsCounterpartyCheck,
    WarningsTransactionCheck,
};

/// Run every EMIR auth.106 data-quality-warnings check.
pub fn run_all_warnings(
    checks: &[Box<dyn WarningsCheck>],
    records: &[TradeWarningsRecord],
    ctx: &CheckContext,
) -> Vec<DqIssue> {
    collect_finalize(checks, ctx, |c| c.run(records, ctx))
}

/// Run every EMIR auth.106 **per-counterparty** `Wrnngs` check.
pub fn run_all_warnings_counterparty(
    checks: &[Box<dyn WarningsCounterpartyCheck>],
    records: &[WarningsCounterpartyRecord],
    ctx: &CheckContext,
) -> Vec<DqIssue> {
    collect_finalize(checks, ctx, |c| c.run(records, ctx))
}

/// Run every EMIR auth.106 **per-UTI** `Wrnngs/TxDtls` check.
pub fn run_all_warnings_transaction(
    checks: &[Box<dyn WarningsTransactionCheck>],
    records: &[WarningsTransactionRecord],
    ctx: &CheckContext,
) -> Vec<DqIssue> {
    collect_finalize(checks, ctx, |c| c.run(records, ctx))
}

// ---- SFTR Missing Collateral Request (auth.083) checks ---------

mod sftr_missing_collateral;

pub use sftr_missing_collateral::{
    default_missing_collateral_checks, MissingCollateralCheck, SftrMcrCollateralStateCrossRef,
    SftrMcrMissingCollateralRequested, SftrMcrMissingUtiOnRequest,
};

/// Run every SFTR auth.083 Missing Collateral Request check. `tsr` is
/// an optional companion SFTR Trade State Report (`--tsr` file or the
/// latest per-UTI state from the history store) for the cross-ref
/// checks; `None` ⇒ those checks no-op.
pub fn run_all_missing_collateral(
    checks: &[Box<dyn MissingCollateralCheck>],
    records: &[MissingCollateralRecord],
    tsr: Option<&[SftrTrStateRecord]>,
    ctx: &CheckContext,
) -> Vec<DqIssue> {
    collect_finalize(checks, ctx, |c| c.run(records, tsr, ctx))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Regime, Severity};

    fn issue(check_id: &str, sev: Severity) -> DqIssue {
        DqIssue {
            check_id: check_id.into(),
            regime: Regime::Emir,
            severity: sev,
            dimension: DqDimension::Completeness,
            record_id: None,
            uti: None,
            field: None,
            value: None,
            message: String::new(),
            source_file: None,
            evidence: Vec::new(),
        }
    }

    #[test]
    fn apply_severity_overrides_replaces_matching_check_ids() {
        let mut t = Thresholds::default();
        t.severity_overrides
            .insert("EMIR.COMP.UTI_MISSING".into(), Severity::Warning);
        let mut issues = vec![
            issue("EMIR.COMP.UTI_MISSING", Severity::High),
            issue("EMIR.UNI.DUPLICATE_UTI", Severity::High),
        ];
        apply_severity_overrides(&mut issues, &t);
        assert_eq!(issues[0].severity, Severity::Warning);
        assert_eq!(issues[1].severity, Severity::High);
    }

    #[test]
    fn apply_severity_overrides_is_a_noop_when_map_empty() {
        let t = Thresholds::default();
        let mut issues = vec![issue("ANY", Severity::High)];
        apply_severity_overrides(&mut issues, &t);
        assert_eq!(issues[0].severity, Severity::High);
    }

    fn msg(check_id: &str, message: &str) -> DqIssue {
        let mut i = issue(check_id, Severity::High);
        i.message = message.into();
        i
    }

    #[test]
    fn sort_issues_tiebreaks_by_message_when_primary_keys_equal() {
        // Same check_id / source_file (None) / record_id (None) — the
        // historical key ties; the content tiebreak orders by message.
        let mut v = vec![msg("EMIR.X", "zebra"), msg("EMIR.X", "alpha")];
        sort_issues(&mut v);
        assert_eq!(v[0].message, "alpha");
        assert_eq!(v[1].message, "zebra");
    }

    #[test]
    fn sort_issues_is_idempotent() {
        let mut once = vec![
            msg("EMIR.B", "m2"),
            msg("EMIR.A", "m9"),
            msg("EMIR.A", "m1"),
            msg("EMIR.B", "m0"),
        ];
        sort_issues(&mut once);
        let mut twice = once.clone();
        sort_issues(&mut twice);
        let key = |i: &[DqIssue]| -> Vec<(String, String)> {
            i.iter()
                .map(|x| (x.check_id.clone(), x.message.clone()))
                .collect()
        };
        assert_eq!(key(&once), key(&twice));
    }

    #[test]
    fn sort_issues_deterministic_total_order_on_known_vector() {
        // check_id is the primary key; message the tiebreak within it.
        let mut v = vec![
            msg("EMIR.B", "b"),
            msg("EMIR.A", "z"),
            msg("EMIR.A", "a"),
            msg("EMIR.B", "a"),
        ];
        sort_issues(&mut v);
        let got: Vec<(&str, &str)> = v
            .iter()
            .map(|i| (i.check_id.as_str(), i.message.as_str()))
            .collect();
        assert_eq!(
            got,
            vec![
                ("EMIR.A", "a"),
                ("EMIR.A", "z"),
                ("EMIR.B", "a"),
                ("EMIR.B", "b"),
            ]
        );
    }

    #[test]
    fn sort_issues_record_id_outranks_message() {
        // record_id is a higher key than message: r1/zzz must precede
        // r2/aaa even though "zzz" > "aaa".
        let mut a = msg("EMIR.X", "zzz");
        a.record_id = Some("r1".into());
        let mut b = msg("EMIR.X", "aaa");
        b.record_id = Some("r2".into());
        let mut v = vec![b, a];
        sort_issues(&mut v);
        assert_eq!(v[0].record_id.as_deref(), Some("r1"));
        assert_eq!(v[1].record_id.as_deref(), Some("r2"));
    }

    #[test]
    fn collect_finalize_preserves_every_issue_and_finalizes() {
        let ctx = CheckContext::now_with_defaults();
        // Three "checks" (plain indices); each emits two issues in a
        // deliberately UN-sorted order to prove the result is the
        // deterministic finalized order, not the production order.
        let checks: Vec<u32> = vec![0, 1, 2];
        let out = collect_finalize(&checks, &ctx, |i| {
            vec![
                msg("EMIR.Z", &format!("chk{i}-z")),
                msg("EMIR.A", &format!("chk{i}-a")),
            ]
        });
        // No issue dropped or duplicated: 3 checks × 2 = 6.
        assert_eq!(out.len(), 6);
        // Result equals the globally finalize-sorted order regardless
        // of per-check emission order / parallel collect.
        let got: Vec<(String, String)> = out
            .iter()
            .map(|i| (i.check_id.clone(), i.message.clone()))
            .collect();
        let mut want = got.clone();
        want.sort();
        assert_eq!(got, want, "collect_finalize must apply finalize_issues");
        // All EMIR.A precede all EMIR.Z (primary key = check_id).
        assert!(out[..3].iter().all(|i| i.check_id == "EMIR.A"));
        assert!(out[3..].iter().all(|i| i.check_id == "EMIR.Z"));
    }

    #[test]
    fn collect_finalize_handles_empty_checks() {
        let ctx = CheckContext::now_with_defaults();
        let checks: Vec<u32> = Vec::new();
        let out = collect_finalize(&checks, &ctx, |_| vec![msg("EMIR.A", "x")]);
        assert!(out.is_empty());
    }

    fn epoch() -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::from_timestamp(0, 0).unwrap()
    }

    #[test]
    fn stream_checks_into_equals_finalize_issues() {
        // The load-bearing D1 invariant: streaming every check's batch
        // into a `SortedIssueSink` then `finish`ing yields exactly the
        // `finalize_issues`-sorted, override-applied union — empty
        // batches skipped harmlessly, severity overrides applied once.
        struct Batch(Vec<DqIssue>);
        let mk = || -> Vec<Box<Batch>> {
            vec![
                Box::new(Batch(vec![
                    issue("EMIR.Z", Severity::High),
                    issue("EMIR.A", Severity::Warning),
                ])),
                Box::new(Batch(Vec::new())), // empty: must be skipped
                Box::new(Batch(vec![
                    issue("EMIR.A", Severity::High),
                    issue("EMIR.M", Severity::Critical),
                ])),
            ]
        };
        let mut t = Thresholds::default();
        t.severity_overrides
            .insert("EMIR.M".into(), Severity::Warning);
        let ctx = CheckContext {
            thresholds: t.clone(),
            ..CheckContext::now_with_defaults()
        };

        // Streaming path.
        let checks = mk();
        let sink = std::sync::Mutex::new(SortedIssueSink::new(&t, STREAM_SPILL_MAX_ISSUES));
        stream_checks_into(&checks, &sink, |c| c.0.clone());
        let (summary, sorted) =
            sink.into_inner()
                .unwrap()
                .finish(Regime::Emir, 1, 2, epoch(), epoch());
        let streamed: Vec<DqIssue> = sorted.collect();

        // Reference path: concat all batches + finalize_issues.
        let mut reference: Vec<DqIssue> = mk().into_iter().flat_map(|b| b.0).collect();
        finalize_issues(&mut reference, &ctx);

        // `DqIssue` has no `PartialEq`; compare the serialised form
        // (the output-byte proxy used across the sink tests).
        let to_json = |v: &[DqIssue]| -> Vec<String> {
            v.iter()
                .map(|i| serde_json::to_string(i).unwrap())
                .collect()
        };
        assert_eq!(
            to_json(&streamed),
            to_json(&reference),
            "stream path must equal finalize path"
        );
        assert_eq!(summary.issues_total as usize, reference.len());
    }

    #[test]
    fn stream_checks_into_empty_checks_is_noop() {
        let t = Thresholds::default();
        let checks: Vec<Box<[DqIssue]>> = Vec::new();
        let sink = std::sync::Mutex::new(SortedIssueSink::new(&t, STREAM_SPILL_MAX_ISSUES));
        stream_checks_into(&checks, &sink, |c: &[DqIssue]| c.to_vec());
        let (summary, sorted) =
            sink.into_inner()
                .unwrap()
                .finish(Regime::Emir, 0, 0, epoch(), epoch());
        assert_eq!(sorted.count(), 0);
        assert_eq!(summary.issues_total, 0);
    }
}
