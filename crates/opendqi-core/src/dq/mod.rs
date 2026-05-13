//! Data-quality check trait and the MVP EMIR check registry.

use chrono::{DateTime, NaiveDate, Utc};

use crate::config::Thresholds;
use crate::model::{DqDimension, DqIssue, EmirRecord, Severity, SftrRecord};

mod abnormal_maturity;
mod cleared_requires_ccp;
mod counterparty_1_missing;
mod counterparty_2_missing;
mod currency_notional;
mod currency_valuation;
mod duplicate_uti;
mod formats;
mod late_reporting;
mod lei_format_err;
mod lei_format_oc;
mod lei_format_rc;
mod missing_uti;
mod missing_valuation;
mod negative_notional;
mod notional_currency_missing;
mod reporting_before_execution;
mod valuation_after_reporting;
mod valuation_after_termination;
mod valuation_currency_missing;
mod valuation_timestamp_missing;
mod zero_notional;

pub use abnormal_maturity::AbnormalMaturity;
pub use cleared_requires_ccp::ClearedRequiresCcp;
pub use counterparty_1_missing::Counterparty1Missing;
pub use counterparty_2_missing::Counterparty2Missing;
pub use currency_notional::CurrencyNotional;
pub use currency_valuation::CurrencyValuation;
pub use duplicate_uti::DuplicateUti;
pub use late_reporting::LateReporting;
pub use lei_format_err::LeiFormatErr;
pub use lei_format_oc::LeiFormatOc;
pub use lei_format_rc::LeiFormatRc;
pub use missing_uti::MissingUti;
pub use missing_valuation::MissingValuation;
pub use negative_notional::NegativeNotional;
pub use notional_currency_missing::NotionalCurrencyMissing;
pub use reporting_before_execution::ReportingBeforeExecution;
pub use valuation_after_reporting::ValuationAfterReporting;
pub use valuation_after_termination::ValuationAfterTermination;
pub use valuation_currency_missing::ValuationCurrencyMissing;
pub use valuation_timestamp_missing::ValuationTimestampMissing;
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
        // Validity
        Box::new(LeiFormatRc),
        Box::new(LeiFormatOc),
        Box::new(LeiFormatErr),
        Box::new(CurrencyNotional),
        Box::new(CurrencyValuation),
        // Accuracy
        Box::new(AbnormalMaturity),
        Box::new(ZeroNotional),
        Box::new(NegativeNotional),
        // Uniqueness
        Box::new(DuplicateUti),
        // Timeliness
        Box::new(LateReporting),
        Box::new(ValuationAfterReporting),
        // Consistency
        Box::new(ReportingBeforeExecution),
        Box::new(ClearedRequiresCcp),
        Box::new(ValuationAfterTermination),
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
    SftrCheck, SftrCollateralCurrencyMissing, SftrCollateralValueMissing, SftrCounterparty1Missing,
    SftrCounterparty2Missing, SftrCurrencyCollateral, SftrCurrencyLoan, SftrDuplicateUti,
    SftrHaircutMissing, SftrHaircutOutOfRange, SftrIsinCollateral, SftrLateReporting,
    SftrLeiFormatErr, SftrLeiFormatOc, SftrLeiFormatRc, SftrLoanCurrencyMissing,
    SftrMaturityBeforeEffective, SftrMissingUti, SftrNegativeCollateral, SftrNegativeLoan,
    SftrSettlementBeforeExecution,
};

/// Default SFTR check registry. 20 checks total covering all six DQ
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
