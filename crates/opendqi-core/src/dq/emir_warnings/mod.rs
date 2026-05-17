//! EMIR Data-Quality Warnings (auth.106) checks.
//!
//! Stat-oriented: each check ingests the TR-produced report-level
//! missing-valuation / missing-margin-info / abnormal-values rates
//! (derived in `opendqi-xml`) and flags a counterparty/report whose
//! rate **exceeds** the configured maximum. Distinct from the
//! reconciliation-statistics family (`EMIR.RST.*`, auth.091).

use crate::config::WarningsThresholds;
use crate::dq::CheckContext;
use crate::model::{
    DqDimension, DqIssue, Regime, Severity, TradeWarningsRecord, WarningsCounterpartyRecord,
    WarningsTransactionRecord,
};
use rust_decimal::prelude::ToPrimitive;

mod abnormal_values_high;
mod missing_margin_info_high;
mod missing_valuation_high;
mod outdated_margin_info_high;
mod outdated_valuation_high;

mod ctrpty_abnormal_values_high;
mod ctrpty_missing_margin_info_high;
mod ctrpty_missing_valuation_high;
mod ctrpty_outdated_margin_info_high;
mod ctrpty_outdated_valuation_high;

mod tx_abnormal_value;
mod tx_missing_margin;
mod tx_missing_valuation;

pub use abnormal_values_high::EmirWrnAbnormalValuesHigh;
pub use missing_margin_info_high::EmirWrnMissingMarginInfoHigh;
pub use missing_valuation_high::EmirWrnMissingValuationHigh;
pub use outdated_margin_info_high::EmirWrnOutdatedMarginInfoHigh;
pub use outdated_valuation_high::EmirWrnOutdatedValuationHigh;

pub use ctrpty_abnormal_values_high::EmirWrnCtrptyAbnormalValuesHigh;
pub use ctrpty_missing_margin_info_high::EmirWrnCtrptyMissingMarginInfoHigh;
pub use ctrpty_missing_valuation_high::EmirWrnCtrptyMissingValuationHigh;
pub use ctrpty_outdated_margin_info_high::EmirWrnCtrptyOutdatedMarginInfoHigh;
pub use ctrpty_outdated_valuation_high::EmirWrnCtrptyOutdatedValuationHigh;

pub use tx_abnormal_value::EmirWrnTxAbnormalValue;
pub use tx_missing_margin::EmirWrnTxMissingMargin;
pub use tx_missing_valuation::EmirWrnTxMissingValuation;

/// An EMIR auth.106 data-quality-warnings check.
pub trait WarningsCheck: Send + Sync {
    /// Stable identifier, e.g. `EMIR.WRN.MISSING_VALUATION_HIGH`.
    fn id(&self) -> &'static str;
    /// The DQ dimension this check belongs to.
    fn dimension(&self) -> DqDimension;
    /// Default severity for issues raised by this check.
    fn severity(&self) -> Severity;
    /// Execute the check against the current batch.
    fn run(&self, records: &[TradeWarningsRecord], ctx: &CheckContext) -> Vec<DqIssue>;
}

/// Default EMIR auth.106 check registry (5 checks).
pub fn default_warnings_checks() -> Vec<Box<dyn WarningsCheck>> {
    vec![
        Box::new(EmirWrnMissingValuationHigh),
        Box::new(EmirWrnOutdatedValuationHigh),
        Box::new(EmirWrnMissingMarginInfoHigh),
        Box::new(EmirWrnOutdatedMarginInfoHigh),
        Box::new(EmirWrnAbnormalValuesHigh),
    ]
}

/// An EMIR auth.106 **per-counterparty** `Wrnngs` check. Same rate
/// semantics as [`WarningsCheck`], applied to one
/// `WarningsCounterpartyRecord` per `(RefDt, CtrPty LEI)`.
pub trait WarningsCounterpartyCheck: Send + Sync {
    /// Stable identifier, e.g. `EMIR.WRN.CTRPTY_MISSING_VALUATION_HIGH`.
    fn id(&self) -> &'static str;
    /// The DQ dimension this check belongs to.
    fn dimension(&self) -> DqDimension;
    /// Default severity for issues raised by this check.
    fn severity(&self) -> Severity;
    /// Execute the check against the per-counterparty batch.
    fn run(&self, records: &[WarningsCounterpartyRecord], ctx: &CheckContext) -> Vec<DqIssue>;
}

/// Default EMIR auth.106 per-counterparty check registry (5 checks).
pub fn default_warnings_counterparty_checks() -> Vec<Box<dyn WarningsCounterpartyCheck>> {
    vec![
        Box::new(EmirWrnCtrptyMissingValuationHigh),
        Box::new(EmirWrnCtrptyOutdatedValuationHigh),
        Box::new(EmirWrnCtrptyMissingMarginInfoHigh),
        Box::new(EmirWrnCtrptyOutdatedMarginInfoHigh),
        Box::new(EmirWrnCtrptyAbnormalValuesHigh),
    ]
}

/// An EMIR auth.106 **per-UTI** `Wrnngs/TxDtls` check. Operational,
/// not statistical: the TR explicitly enumerated this transaction as
/// problematic, so each record yields one issue (same shape as
/// `EMIR.REC.*` / `SFTR.MCR.MISSING_COLLATERAL_REQUESTED`), filtered
/// by `warning_category`.
pub trait WarningsTransactionCheck: Send + Sync {
    /// Stable identifier, e.g. `EMIR.WRN.TX_MISSING_VALUATION`.
    fn id(&self) -> &'static str;
    /// The DQ dimension this check belongs to.
    fn dimension(&self) -> DqDimension;
    /// Default severity for issues raised by this check.
    fn severity(&self) -> Severity;
    /// Execute the check against the per-UTI batch.
    fn run(&self, records: &[WarningsTransactionRecord], ctx: &CheckContext) -> Vec<DqIssue>;
}

/// Default EMIR auth.106 per-UTI check registry (3 checks, one per
/// `Wrnngs/TxDtls` category).
pub fn default_warnings_transaction_checks() -> Vec<Box<dyn WarningsTransactionCheck>> {
    vec![
        Box::new(EmirWrnTxMissingValuation),
        Box::new(EmirWrnTxMissingMargin),
        Box::new(EmirWrnTxAbnormalValue),
    ]
}

/// Build a `DqIssue` shell for a per-UTI `WarningsTransactionRecord`
/// (sets `uti`; callers override `message`). Mirrors
/// `sftr_missing_collateral::build_issue`.
pub(crate) fn build_issue_tx(
    check_id: &str,
    severity: Severity,
    dimension: DqDimension,
    rec: &WarningsTransactionRecord,
) -> DqIssue {
    DqIssue {
        check_id: check_id.into(),
        regime: Regime::Emir,
        severity,
        dimension,
        record_id: rec.record_id.clone(),
        uti: rec.uti.clone(),
        field: None,
        value: None,
        message: String::new(),
        source_file: rec.source_file.clone(),
        evidence: Vec::new(),
    }
}

/// Counterparty pair label for per-UTI issue messages.
pub(crate) fn tx_counterparty_label(rec: &WarningsTransactionRecord) -> String {
    format!(
        "{} vs {}",
        rec.counterparty_lei.as_deref().unwrap_or("(no LEI)"),
        rec.other_counterparty
            .as_deref()
            .unwrap_or("(no OthrCtrPty)"),
    )
}

/// Decimal → f64 lossy converter; safe for rates in `[0.0, 1.0]`.
pub(crate) fn decimal_to_f64(d: rust_decimal::Decimal) -> f64 {
    d.to_f64().unwrap_or_default()
}

/// Format the reporting-date label for issue messages.
pub(crate) fn reporting_date_label(rec: &TradeWarningsRecord) -> String {
    rec.reporting_date
        .map(|d| d.to_string())
        .unwrap_or_else(|| "(no date)".into())
}

/// Build a `DqIssue` shell pre-populated with common fields. Callers
/// override `field`, `value`, `message`.
pub(crate) fn build_issue(
    check_id: &str,
    severity: Severity,
    dimension: DqDimension,
    rec: &TradeWarningsRecord,
) -> DqIssue {
    DqIssue {
        check_id: check_id.into(),
        regime: Regime::Emir,
        severity,
        dimension,
        record_id: rec.record_id.clone(),
        uti: None,
        field: None,
        value: None,
        message: String::new(),
        source_file: rec.source_file.clone(),
        evidence: Vec::new(),
    }
}

/// Borrow the warnings thresholds from a `CheckContext`.
pub(crate) fn thresholds(ctx: &CheckContext) -> &WarningsThresholds {
    &ctx.thresholds.warnings
}

/// Shared "rate exceeds max" check body.
#[allow(clippy::too_many_arguments)]
pub(crate) fn flag_rate_high(
    check_id: &str,
    severity: Severity,
    dimension: DqDimension,
    field: &str,
    label: &str,
    records: &[TradeWarningsRecord],
    rate_of: impl Fn(&TradeWarningsRecord) -> Option<rust_decimal::Decimal>,
    max: f64,
) -> Vec<DqIssue> {
    records
        .iter()
        .filter_map(|r| {
            let rate = rate_of(r).map(decimal_to_f64)?;
            if rate <= max {
                return None;
            }
            let mut issue = build_issue(check_id, severity, dimension, r);
            issue.field = Some(field.to_string());
            issue.value = Some(format!("{rate:.4}"));
            issue.message = format!(
                "{label} rate {:.2}% > threshold {:.2}% on {}.",
                rate * 100.0,
                max * 100.0,
                reporting_date_label(r),
            );
            Some(issue)
        })
        .collect()
}

/// Shared per-counterparty "rate exceeds max" check body. Mirrors
/// [`flag_rate_high`] but over `WarningsCounterpartyRecord`, and names
/// the offending counterparty LEI in the issue.
#[allow(clippy::too_many_arguments)]
pub(crate) fn flag_rate_high_cp(
    check_id: &str,
    severity: Severity,
    dimension: DqDimension,
    field: &str,
    label: &str,
    records: &[WarningsCounterpartyRecord],
    rate_of: impl Fn(&WarningsCounterpartyRecord) -> Option<rust_decimal::Decimal>,
    max: f64,
) -> Vec<DqIssue> {
    records
        .iter()
        .filter_map(|r| {
            let rate = rate_of(r).map(decimal_to_f64)?;
            if rate <= max {
                return None;
            }
            let lei = r.counterparty_lei.as_deref().unwrap_or("(no LEI)");
            let date = r
                .reporting_date
                .map(|d| d.to_string())
                .unwrap_or_else(|| "(no date)".into());
            Some(DqIssue {
                check_id: check_id.into(),
                regime: Regime::Emir,
                severity,
                dimension,
                record_id: r.record_id.clone(),
                uti: None,
                field: Some(field.to_string()),
                value: Some(format!("{rate:.4}")),
                message: format!(
                    "{label} rate {:.2}% > threshold {:.2}% for counterparty {lei} on {date}.",
                    rate * 100.0,
                    max * 100.0,
                ),
                source_file: r.source_file.clone(),
                evidence: Vec::new(),
            })
        })
        .collect()
}
