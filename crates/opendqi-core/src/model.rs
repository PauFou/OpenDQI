//! Canonical OpenDQI domain types.

use std::collections::BTreeMap;

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Regulatory regime under inspection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Regime {
    /// European Market Infrastructure Regulation.
    Emir,
    /// Securities Financing Transactions Regulation.
    Sftr,
}

impl std::fmt::Display for Regime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Regime::Emir => f.write_str("emir"),
            Regime::Sftr => f.write_str("sftr"),
        }
    }
}

/// Severity ranks issues from informational to blocking.
///
/// Ordering is `Info < Warning < High < Critical` — preserved for
/// deterministic iteration over `BTreeMap<Severity, _>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Informational note, no action required.
    Info,
    /// Mild anomaly; worth investigating.
    Warning,
    /// Significant data-quality defect.
    High,
    /// Blocking defect; record must be remediated before resubmission.
    Critical,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Info => f.write_str("info"),
            Severity::Warning => f.write_str("warning"),
            Severity::High => f.write_str("high"),
            Severity::Critical => f.write_str("critical"),
        }
    }
}

/// The classical six-pillar taxonomy of data-quality dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DqDimension {
    /// All required fields are present.
    Completeness,
    /// Identifiers are unique where they should be.
    Uniqueness,
    /// Reports arrive within deadlines.
    Timeliness,
    /// Values respect the technical format / schema.
    Validity,
    /// Values are correct with respect to reality.
    Accuracy,
    /// Related fields agree with one another.
    Consistency,
}

impl std::fmt::Display for DqDimension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DqDimension::Completeness => f.write_str("completeness"),
            DqDimension::Uniqueness => f.write_str("uniqueness"),
            DqDimension::Timeliness => f.write_str("timeliness"),
            DqDimension::Validity => f.write_str("validity"),
            DqDimension::Accuracy => f.write_str("accuracy"),
            DqDimension::Consistency => f.write_str("consistency"),
        }
    }
}

/// A single data-quality finding.
///
/// All optional fields are populated on a best-effort basis so that the
/// resulting issue is self-describing even when read in isolation
/// (e.g. inside `issues.csv`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DqIssue {
    /// Stable identifier of the check that produced this issue,
    /// e.g. `EMIR.COMP.UTI_MISSING`.
    pub check_id: String,
    /// Regulatory regime.
    pub regime: Regime,
    /// Severity rank.
    pub severity: Severity,
    /// Data-quality dimension.
    pub dimension: DqDimension,
    /// Source record identifier, when known.
    pub record_id: Option<String>,
    /// Trade UTI, when known.
    pub uti: Option<String>,
    /// Field name implicated by the issue, when applicable.
    pub field: Option<String>,
    /// Field value as a string, when applicable.
    pub value: Option<String>,
    /// Human-readable message describing the finding.
    pub message: String,
    /// Source file path the record came from, when known.
    pub source_file: Option<String>,
}

/// Canonical EMIR record.
///
/// Almost every field is optional: real-world reports omit many fields,
/// and the data-quality checks are precisely the mechanism that flags
/// those gaps.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EmirRecord {
    /// Source file path (or other origin label).
    pub source_file: Option<String>,
    /// Stable identifier of the record within the source.
    pub record_id: Option<String>,
    /// Unique Trade Identifier.
    pub uti: Option<String>,
    /// Prior UTI when the trade has been re-identified.
    pub prior_uti: Option<String>,
    /// EMIR action type (NEWT, MODI, TERM, CORR, ...).
    pub action_type: Option<String>,
    /// EMIR event type.
    pub event_type: Option<String>,
    /// Entity Responsible for Reporting (LEI).
    pub entity_responsible_for_reporting: Option<String>,
    /// Counterparty 1 (LEI).
    pub counterparty_1: Option<String>,
    /// Counterparty 2 (LEI).
    pub counterparty_2: Option<String>,
    /// Asset class code.
    pub asset_class: Option<String>,
    /// Product identifier (e.g. ISIN, classification).
    pub product_id: Option<String>,
    /// Underlying identifier.
    pub underlying_id: Option<String>,
    /// Notional amount.
    pub notional_amount: Option<Decimal>,
    /// Notional currency (ISO 4217).
    pub notional_currency: Option<String>,
    /// Price.
    pub price: Option<Decimal>,
    /// Price currency.
    pub price_currency: Option<String>,
    /// Execution timestamp.
    pub execution_timestamp: Option<DateTime<Utc>>,
    /// Event timestamp.
    pub event_timestamp: Option<DateTime<Utc>>,
    /// Reporting timestamp.
    pub reporting_timestamp: Option<DateTime<Utc>>,
    /// Effective date.
    pub effective_date: Option<NaiveDate>,
    /// Maturity date.
    pub maturity_date: Option<NaiveDate>,
    /// Termination date.
    pub termination_date: Option<NaiveDate>,
    /// Valuation amount.
    pub valuation_amount: Option<Decimal>,
    /// Valuation currency.
    pub valuation_currency: Option<String>,
    /// Valuation timestamp.
    pub valuation_timestamp: Option<DateTime<Utc>>,
    /// Initial margin posted.
    pub initial_margin_posted: Option<Decimal>,
    /// Initial margin collected.
    pub initial_margin_collected: Option<Decimal>,
    /// Variation margin posted.
    pub variation_margin_posted: Option<Decimal>,
    /// Variation margin collected.
    pub variation_margin_collected: Option<Decimal>,
    /// Collateral portfolio code.
    pub collateral_portfolio_code: Option<String>,
    /// Clearing status.
    pub clearing_status: Option<String>,
    /// Collateralisation category.
    pub collateralisation_category: Option<String>,
}

impl EmirRecord {
    /// Returns true when the trade is considered outstanding, i.e. not
    /// terminated as of `today`.
    pub fn is_outstanding(&self, today: NaiveDate) -> bool {
        match self.termination_date {
            None => true,
            Some(t) => t > today,
        }
    }
}

/// Aggregate statistics for a scan run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanSummary {
    /// Regulatory regime scanned.
    pub regime: Regime,
    /// Number of input files processed.
    pub files_processed: u32,
    /// Number of records examined.
    pub records_processed: u32,
    /// Total issues raised.
    pub issues_total: u32,
    /// Issue counts grouped by severity.
    pub issues_by_severity: BTreeMap<Severity, u32>,
    /// Issue counts grouped by dimension.
    pub issues_by_dimension: BTreeMap<DqDimension, u32>,
    /// Overall quality score on a 0–100 scale (higher is better).
    pub quality_score: f32,
    /// Scan start timestamp.
    pub started_at: DateTime<Utc>,
    /// Scan completion timestamp.
    pub finished_at: DateTime<Utc>,
}
