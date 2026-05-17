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

/// One discrete piece of evidence supporting a `DqIssue`. Captures
/// the `field`, the `before` / `after` values for cross-batch
/// comparisons (e.g. MODI lifecycle), and the originating
/// `source_line` when the record's index in its source file is
/// known. All fields are optional so checks can populate only what
/// is meaningful.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceItem {
    /// Field name the evidence relates to.
    pub field: String,
    /// Value observed before the change (or in the prior batch).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    /// Value observed after the change (or in the current batch).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    /// Line number / index in the source file when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_line: Option<u64>,
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
    /// Structured supporting evidence. Empty by default; populated by
    /// checks where `before` / `after` comparisons, source lines, or
    /// per-field details add audit value (e.g. duplicate-UTI source
    /// lines, MODI-preserves-UTI before/after).
    #[serde(default)]
    pub evidence: Vec<EvidenceItem>,
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
    /// Notional amount of the second leg (swap-like products).
    pub leg2_notional_amount: Option<Decimal>,
    /// Currency of the second-leg notional.
    pub leg2_notional_currency: Option<String>,
    /// Payment frequency of the first leg (free-form code).
    pub leg1_payment_frequency: Option<String>,
    /// Payment frequency of the second leg.
    pub leg2_payment_frequency: Option<String>,
    /// LEI of the Central Counterparty when the trade is cleared.
    pub clearing_ccp_lei: Option<String>,
    /// Intragroup transaction indicator.
    pub intragroup_indicator: Option<bool>,
    /// Hedging indicator (directly linked to commercial activity).
    pub hedging_indicator: Option<bool>,
    /// Valuation type ("MTMA" mark-to-market, "MTMO" mark-to-model).
    pub valuation_type: Option<String>,
    /// Trading capacity (AGEN / PRIN / ...).
    pub trading_capacity: Option<String>,
    /// Commercial or treasury financing indicator.
    pub commercial_or_treasury_financing: Option<bool>,
    /// Reporting obligation indicator.
    pub reporting_obligation_indicator: Option<String>,
    /// Corporate sector of the reporting counterparty.
    pub corporate_sector: Option<String>,
    /// Nature of the reporting counterparty (FC / NFCM / NFC ...).
    pub nature: Option<String>,
    /// Master agreement type (e.g. ISDA).
    pub master_agreement_type: Option<String>,
    /// Master agreement version (e.g. 2002).
    pub master_agreement_version: Option<String>,
    /// Confirmation method.
    pub confirmation_method: Option<String>,
    /// Mark-to-market value change since previous valuation.
    pub mtm_value_change: Option<Decimal>,
    /// Option greek: delta.
    pub delta: Option<Decimal>,
    /// Option greek: gamma.
    pub gamma: Option<Decimal>,
    /// Option greek: vega.
    pub vega: Option<Decimal>,
    /// Source system identifier from the report envelope (e.g. submitter
    /// LEI/BIC, or an internal upstream system tag). Populated on a
    /// best-effort basis from the XML envelope or CSV mapping when
    /// present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_system: Option<String>,
    /// Catch-all map of any source-format leaf that did not match a
    /// typed field. Key is the source-format-relative path
    /// (e.g. `CmonTradData/CtrctData/UndrlygInstrm/.../X`), value is
    /// the leaf text — optionally suffixed with attribute hints
    /// (e.g. `"1500.50|Ccy=EUR"`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub raw_fields: BTreeMap<String, String>,
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

/// Canonical SFTR record.
///
/// Parallel to [`EmirRecord`]: a Securities Financing Transaction
/// reportable under the EU Securities Financing Transactions
/// Regulation. Like the EMIR variant, almost every field is optional
/// — the data-quality checks surface the gaps.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SftrRecord {
    /// Source file path (or other origin label).
    pub source_file: Option<String>,
    /// Stable identifier of the record within the source.
    pub record_id: Option<String>,

    /// Unique Trade Identifier.
    pub uti: Option<String>,
    /// Prior UTI after a re-identification.
    pub prior_uti: Option<String>,

    /// SFTR action type (NEWT, MODI, CORR, ETRM, VALU, COLU, REUU,
    /// POSC, MARU, OTHR).
    pub action_type: Option<String>,
    /// SFTR event type code.
    pub event_type: Option<String>,

    /// Entity Responsible for Reporting (LEI).
    pub entity_responsible_for_reporting: Option<String>,
    /// Reporting counterparty (LEI).
    pub counterparty_1: Option<String>,
    /// Other counterparty (LEI / BIC).
    pub counterparty_2: Option<String>,

    /// SFT type code: `REPO`, `BSB` (buy-sell-back), `SLEB`
    /// (securities lending or borrowing), `MGLD` (margin lending).
    pub sft_type: Option<String>,
    /// Master agreement type (GMRA, GMSLA, ...).
    pub master_agreement_type: Option<String>,
    /// Master agreement version (e.g. "2011").
    pub master_agreement_version: Option<String>,

    /// Loan / principal value.
    pub loan_value: Option<Decimal>,
    /// Loan / principal currency (ISO 4217).
    pub loan_currency: Option<String>,
    /// Collateral value.
    pub collateral_value: Option<Decimal>,
    /// Collateral currency.
    pub collateral_currency: Option<String>,
    /// Haircut applied to the collateral.
    pub haircut: Option<Decimal>,
    /// "Available for collateral reuse" indicator.
    pub reuse_indicator: Option<bool>,
    /// Repo rebate rate.
    pub rebate_rate: Option<Decimal>,
    /// Lending fee.
    pub lending_fee: Option<Decimal>,

    /// Execution timestamp.
    pub execution_timestamp: Option<DateTime<Utc>>,
    /// Event timestamp.
    pub event_timestamp: Option<DateTime<Utc>>,
    /// Reporting timestamp.
    pub reporting_timestamp: Option<DateTime<Utc>>,

    /// Effective / event date.
    pub effective_date: Option<NaiveDate>,
    /// Maturity date.
    pub maturity_date: Option<NaiveDate>,
    /// Termination date.
    pub termination_date: Option<NaiveDate>,
    /// Settlement date.
    pub settlement_date: Option<NaiveDate>,

    /// Collateral portfolio code.
    pub collateral_portfolio_code: Option<String>,
    /// ISIN of the security used as collateral.
    pub collateral_isin: Option<String>,
    /// ISIN (or other identifier) of the security being lent / borrowed —
    /// the principal SFT leg, distinct from the collateral leg.
    /// Populated for `SLEB`, `SBSC`, `BSB` typed SFTs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub security_identifier: Option<String>,

    /// Catch-all of XML leaves that were not promoted to typed fields.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub raw_fields: BTreeMap<String, String>,
}

impl SftrRecord {
    /// Returns true when the SFT is considered outstanding, i.e. not
    /// terminated as of `today`.
    pub fn is_outstanding(&self, today: NaiveDate) -> bool {
        match self.termination_date {
            None => true,
            Some(t) => t > today,
        }
    }
}

/// Type of feedback a Trade Repository sends back about a previously
/// submitted report.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackType {
    /// TR rejected the report at ingestion (validation failure).
    #[default]
    Rejected,
    /// TR signals the UTI is missing from its records.
    Missing,
    /// TR accepted the report but flagged inaccurate fields.
    Inaccurate,
    /// TR signals a reconciliation break with the counterparty's submission.
    ReconciliationBreak,
}

impl std::fmt::Display for FeedbackType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FeedbackType::Rejected => f.write_str("rejected"),
            FeedbackType::Missing => f.write_str("missing"),
            FeedbackType::Inaccurate => f.write_str("inaccurate"),
            FeedbackType::ReconciliationBreak => f.write_str("reconciliation_break"),
        }
    }
}

/// One feedback line item from a Trade Repository auth.092 (EMIR) or
/// auth.080 (SFTR) message. Each record references a UTI the firm
/// previously submitted, along with the TR's diagnosis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackRecord {
    /// Source file path (or other origin label).
    pub source_file: Option<String>,
    /// Stable identifier of the line within the source (e.g. Sts index).
    pub record_id: Option<String>,
    /// Regulatory regime this feedback applies to.
    pub regime: Regime,
    /// Type of feedback signalled by the TR.
    pub feedback_type: FeedbackType,
    /// UTI of the submission the feedback refers to.
    pub uti: Option<String>,
    /// Machine-readable reason code (e.g. `VAL01`). Kept for backward
    /// compatibility — equals the **first** of `validation_rule_codes`.
    pub reason_code: Option<String>,
    /// All TR validation-rule codes for this record. `auth.092` lists
    /// several `DtldVldtnRule` per rejected transaction; this is the
    /// faithful list (`reason_code` is its first element).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub validation_rule_codes: Vec<String>,
    /// Human-readable reason description provided by the TR.
    pub reason_description: Option<String>,
    /// For `Inaccurate` feedback: which field is flagged.
    pub reported_field: Option<String>,
    /// Timestamp of the feedback message itself.
    pub feedback_timestamp: Option<DateTime<Utc>>,
}

impl Default for FeedbackRecord {
    fn default() -> Self {
        Self {
            source_file: None,
            record_id: None,
            regime: Regime::Emir,
            feedback_type: FeedbackType::default(),
            uti: None,
            reason_code: None,
            validation_rule_codes: Vec::new(),
            reason_description: None,
            reported_field: None,
            feedback_timestamp: None,
        }
    }
}

/// Action-type / event-type distributions over a TR Trade Activity
/// Report batch. Embedded in the activity scan's `summary.json` so
/// the report can render histogram tables without re-scanning.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrActivitySummary {
    /// Number of records in the batch.
    pub total_records: u32,
    /// Counts of records by `action_type` (e.g. `NEWT` → 12).
    pub action_distribution: BTreeMap<String, u32>,
    /// Counts of records by `event_type`.
    pub event_distribution: BTreeMap<String, u32>,
}

/// One line from a Trade Repository Trade State Report (TSR):
/// the TR's view of one outstanding trade at a given point in time
/// (ISO 20022 `auth.107` for EMIR; `auth.079` for SFTR is on the
/// Phase 6 roadmap).
///
/// A TSR is **state-oriented**, not activity-oriented: every line
/// describes what the TR currently believes is outstanding, not how
/// the state changed. The companion `state_as_of` timestamp from
/// the report header is propagated to every record so staleness
/// checks have a deterministic reference clock.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrStateRecord {
    /// Source file path (or other origin label).
    pub source_file: Option<String>,
    /// Stable identifier of the line within the source.
    pub record_id: Option<String>,
    /// Regulatory regime this state belongs to.
    pub regime: Regime,
    /// Snapshot timestamp from the report header — every record in
    /// the same TSR shares this value.
    pub state_as_of: Option<DateTime<Utc>>,
    /// UTI of the outstanding trade.
    pub uti: Option<String>,
    /// LEI of the reporting counterparty.
    pub reporting_counterparty: Option<String>,
    /// LEI of the other counterparty.
    pub other_counterparty: Option<String>,
    /// TR-side status — typically `OUTSTANDING`, `MATURED`,
    /// `TERMINATED`, or a TR-specific code.
    pub status: Option<String>,
    /// Notional amount.
    pub notional_amount: Option<Decimal>,
    /// Notional currency (ISO 4217).
    pub notional_currency: Option<String>,
    /// Latest valuation amount the TR is holding.
    pub valuation_amount: Option<Decimal>,
    /// Currency of the latest valuation amount.
    pub valuation_currency: Option<String>,
    /// Timestamp of the latest valuation the TR is holding.
    pub valuation_timestamp: Option<DateTime<Utc>>,
    /// Effective / event date.
    pub effective_date: Option<NaiveDate>,
    /// Contractual maturity date.
    pub maturity_date: Option<NaiveDate>,
    /// Termination date, when applicable.
    pub termination_date: Option<NaiveDate>,
    /// Collateral portfolio code.
    pub collateral_portfolio_code: Option<String>,
    /// Catch-all of XML leaves not promoted to typed fields.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub raw_fields: BTreeMap<String, String>,
}

impl Default for TrStateRecord {
    fn default() -> Self {
        Self {
            source_file: None,
            record_id: None,
            regime: Regime::Emir,
            state_as_of: None,
            uti: None,
            reporting_counterparty: None,
            other_counterparty: None,
            status: None,
            notional_amount: None,
            notional_currency: None,
            valuation_amount: None,
            valuation_currency: None,
            valuation_timestamp: None,
            effective_date: None,
            maturity_date: None,
            termination_date: None,
            collateral_portfolio_code: None,
            raw_fields: BTreeMap::new(),
        }
    }
}

/// One line from an SFTR Trade State Report (TSR) — the TR's view
/// of one outstanding securities-financing transaction at a given
/// point in time (ISO 20022 `auth.079`).
///
/// Like `TrStateRecord` but with SFT-specific fields:
/// loan / collateral / haircut / sft_type / reuse_indicator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SftrTrStateRecord {
    /// Source file path (or other origin label).
    pub source_file: Option<String>,
    /// Stable identifier of the line within the source.
    pub record_id: Option<String>,
    /// Regulatory regime — always `Regime::Sftr` for this record.
    pub regime: Regime,
    /// Snapshot timestamp from the report header.
    pub state_as_of: Option<DateTime<Utc>>,
    /// UTI of the outstanding SFT.
    pub uti: Option<String>,
    /// LEI of the reporting counterparty.
    pub reporting_counterparty: Option<String>,
    /// LEI of the other counterparty.
    pub other_counterparty: Option<String>,
    /// TR-side status — typically `OUTSTANDING`, `MATURED`,
    /// `TERMINATED`.
    pub status: Option<String>,
    /// SFT type: `REPO`, `BSB`, `SLEB`, `MGLD`.
    pub sft_type: Option<String>,
    /// Loan / principal value (latest TR-held amount).
    pub loan_value: Option<Decimal>,
    /// Loan currency (ISO 4217).
    pub loan_currency: Option<String>,
    /// Collateral market value (latest TR-held amount).
    pub collateral_value: Option<Decimal>,
    /// Collateral currency.
    pub collateral_currency: Option<String>,
    /// Haircut applied to the collateral (0.0 — 1.0).
    pub haircut: Option<Decimal>,
    /// "Available for collateral reuse" indicator.
    pub reuse_indicator: Option<bool>,
    /// Effective / event date.
    pub effective_date: Option<NaiveDate>,
    /// Maturity date.
    pub maturity_date: Option<NaiveDate>,
    /// Termination date, when applicable.
    pub termination_date: Option<NaiveDate>,
    /// Settlement date.
    pub settlement_date: Option<NaiveDate>,
    /// Collateral portfolio code.
    pub collateral_portfolio_code: Option<String>,
    /// ISIN of the security used as collateral.
    pub collateral_isin: Option<String>,
    /// Catch-all of XML leaves not promoted to typed fields.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub raw_fields: BTreeMap<String, String>,
}

impl Default for SftrTrStateRecord {
    fn default() -> Self {
        Self {
            source_file: None,
            record_id: None,
            regime: Regime::Sftr,
            state_as_of: None,
            uti: None,
            reporting_counterparty: None,
            other_counterparty: None,
            status: None,
            sft_type: None,
            loan_value: None,
            loan_currency: None,
            collateral_value: None,
            collateral_currency: None,
            haircut: None,
            reuse_indicator: None,
            effective_date: None,
            maturity_date: None,
            termination_date: None,
            settlement_date: None,
            collateral_portfolio_code: None,
            collateral_isin: None,
            raw_fields: BTreeMap::new(),
        }
    }
}

/// One line from an EMIR Margin Activity Report (MAR) — the
/// history of margin calls / postings / collections for a portfolio
/// (ISO 20022 `auth.108`). Activity-oriented, mirrors `EmirRecord`
/// in spirit but scoped to margin events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarginActivityRecord {
    /// Source file path (or other origin label).
    pub source_file: Option<String>,
    /// Stable identifier of the line within the source.
    pub record_id: Option<String>,
    /// Regulatory regime — always `Regime::Emir` for v1.
    pub regime: Regime,
    /// UTI of the underlying trade (when known).
    pub uti: Option<String>,
    /// LEI of counterparty 1.
    pub counterparty_1: Option<String>,
    /// LEI of counterparty 2.
    pub counterparty_2: Option<String>,
    /// Action type — `MARU` (update) / `MARV` (variation) /
    /// `MARC` (correction) / `MARN` (new) typically.
    pub action_type: Option<String>,
    /// Event type — free-text supplementary code.
    pub event_type: Option<String>,
    /// Collateral portfolio code grouping the margin call.
    pub collateral_portfolio_code: Option<String>,
    /// Initial margin posted by counterparty 1.
    pub initial_margin_posted: Option<Decimal>,
    /// Initial margin collected from counterparty 2.
    pub initial_margin_collected: Option<Decimal>,
    /// Variation margin posted by counterparty 1.
    pub variation_margin_posted: Option<Decimal>,
    /// Variation margin collected from counterparty 2.
    pub variation_margin_collected: Option<Decimal>,
    /// ISO 4217 currency for every margin amount on this row.
    pub margin_currency: Option<String>,
    /// Excess collateral above the minimum required.
    pub excess_collateral: Option<Decimal>,
    /// Collateral haircut applied (0.0 – 1.0).
    pub collateral_haircut: Option<Decimal>,
    /// Timestamp of the underlying margin event.
    pub event_timestamp: Option<DateTime<Utc>>,
    /// Timestamp at which the row was reported to the TR.
    pub reporting_timestamp: Option<DateTime<Utc>>,
    /// Catch-all of XML leaves not promoted to typed fields.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub raw_fields: BTreeMap<String, String>,
}

impl Default for MarginActivityRecord {
    fn default() -> Self {
        Self {
            source_file: None,
            record_id: None,
            regime: Regime::Emir,
            uti: None,
            counterparty_1: None,
            counterparty_2: None,
            action_type: None,
            event_type: None,
            collateral_portfolio_code: None,
            initial_margin_posted: None,
            initial_margin_collected: None,
            variation_margin_posted: None,
            variation_margin_collected: None,
            margin_currency: None,
            excess_collateral: None,
            collateral_haircut: None,
            event_timestamp: None,
            reporting_timestamp: None,
            raw_fields: BTreeMap::new(),
        }
    }
}

/// One line from an EMIR Margin State Report (MSR) — the TR's
/// current view of margin postings for an outstanding portfolio
/// (ISO 20022 `auth.109`). State-oriented snapshot, mirrors
/// `TrStateRecord` in spirit but scoped to margin fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarginStateRecord {
    /// Source file path.
    pub source_file: Option<String>,
    /// Stable identifier of the line.
    pub record_id: Option<String>,
    /// Regulatory regime — always `Regime::Emir` for v1.
    pub regime: Regime,
    /// UTI of the underlying outstanding trade.
    pub uti: Option<String>,
    /// LEI of counterparty 1.
    pub counterparty_1: Option<String>,
    /// LEI of counterparty 2.
    pub counterparty_2: Option<String>,
    /// Collateral portfolio code.
    pub collateral_portfolio_code: Option<String>,
    /// Current initial margin posted by counterparty 1.
    pub initial_margin_posted_current: Option<Decimal>,
    /// Current initial margin collected from counterparty 2.
    pub initial_margin_collected_current: Option<Decimal>,
    /// Current variation margin posted by counterparty 1.
    pub variation_margin_posted_current: Option<Decimal>,
    /// Current variation margin collected from counterparty 2.
    pub variation_margin_collected_current: Option<Decimal>,
    /// ISO 4217 currency for every margin amount.
    pub margin_currency: Option<String>,
    /// Current market value of the collateral pool.
    pub collateral_market_value: Option<Decimal>,
    /// Effective haircut on the collateral pool (0.0 – 1.0).
    pub haircut_applied: Option<Decimal>,
    /// `FCOL` (fully) / `PCOL` (partially) / `UCOL` (uncollateralised) /
    /// `OCOL` (one-way).
    pub collateralization_category: Option<String>,
    /// Snapshot timestamp from the report header.
    pub state_as_of: Option<DateTime<Utc>>,
    /// Catch-all.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub raw_fields: BTreeMap<String, String>,
}

impl Default for MarginStateRecord {
    fn default() -> Self {
        Self {
            source_file: None,
            record_id: None,
            regime: Regime::Emir,
            uti: None,
            counterparty_1: None,
            counterparty_2: None,
            collateral_portfolio_code: None,
            initial_margin_posted_current: None,
            initial_margin_collected_current: None,
            variation_margin_posted_current: None,
            variation_margin_collected_current: None,
            margin_currency: None,
            collateral_market_value: None,
            haircut_applied: None,
            collateralization_category: None,
            state_as_of: None,
            raw_fields: BTreeMap::new(),
        }
    }
}

/// One line item from a Trade Repository reconciliation report
/// (ISO 20022 `auth.106` for EMIR, `auth.083` for SFTR). Each
/// record describes a UTI's pairing / reconciliation status between
/// the two counterparties from the TR's perspective.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconciliationRecord {
    /// Source file path (or other origin label).
    pub source_file: Option<String>,
    /// Stable identifier of the line within the source.
    pub record_id: Option<String>,
    /// Regulatory regime this reconciliation applies to.
    pub regime: Regime,
    /// UTI the line refers to.
    pub uti: Option<String>,
    /// LEI of the reporting counterparty.
    pub reporting_counterparty: Option<String>,
    /// LEI of the other counterparty.
    pub other_counterparty: Option<String>,
    /// Pairing status: typically `PAIRED` / `UNPAIRED`.
    pub pairing_status: Option<String>,
    /// Reconciliation status: typically `RECONCILED` / `UNRECONCILED`.
    pub reconciliation_status: Option<String>,
    /// Names of fields the TR flagged as mismatched between the two
    /// counterparties' submissions.
    pub mismatched_fields: Vec<String>,
    /// Timestamp of the reconciliation message itself.
    pub reconciliation_timestamp: Option<DateTime<Utc>>,
}

impl Default for ReconciliationRecord {
    fn default() -> Self {
        Self {
            source_file: None,
            record_id: None,
            regime: Regime::Emir,
            uti: None,
            reporting_counterparty: None,
            other_counterparty: None,
            pairing_status: None,
            reconciliation_status: None,
            mismatched_fields: Vec::new(),
            reconciliation_timestamp: None,
        }
    }
}

/// One line from an EMIR Reconciliation Statistics Report
/// (ISO 20022 `auth.091`). Each record summarises pairing and
/// reconciliation rates for one reporting period and counterparty
/// — TR-produced statistical feedback distinct from the per-trade
/// `auth.106` reconciliation report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconStatsRecord {
    /// Source file path (or other origin label).
    pub source_file: Option<String>,
    /// Stable identifier of the line within the source.
    pub record_id: Option<String>,
    /// Regulatory regime — always `Regime::Emir` for v1.
    pub regime: Regime,
    /// Reporting period end-date the statistics cover.
    pub reporting_date: Option<NaiveDate>,
    /// LEI of the counterparty the statistics relate to.
    pub counterparty_lei: Option<String>,
    /// Pairing rate (0.0 — 1.0): share of submissions paired with the
    /// counterparty's submission.
    pub pairing_rate: Option<Decimal>,
    /// Reconciliation rate (0.0 — 1.0): share of paired submissions
    /// whose fields reconcile.
    pub recon_rate: Option<Decimal>,
    /// Count of outstanding trades paired with the counterparty.
    pub outstanding_paired: Option<i64>,
    /// Count of outstanding trades unpaired (no matching counterparty
    /// submission).
    pub outstanding_unpaired: Option<i64>,
    /// Catch-all of XML leaves that were not promoted to typed fields.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub raw_fields: BTreeMap<String, String>,
}

impl Default for ReconStatsRecord {
    fn default() -> Self {
        Self {
            source_file: None,
            record_id: None,
            regime: Regime::Emir,
            reporting_date: None,
            counterparty_lei: None,
            pairing_rate: None,
            recon_rate: None,
            outstanding_paired: None,
            outstanding_unpaired: None,
            raw_fields: BTreeMap::new(),
        }
    }
}

/// One report-level line from an EMIR Data-Quality Warnings Report
/// (ISO 20022 `auth.106`, `DerivativesTradeWarningsReportV01`). Each
/// record summarises, for one reference date, the TR-produced
/// missing-valuation / missing-margin-info / abnormal-values counts and
/// the rates derived from them. The per-counterparty `Wrnngs` detail
/// is a documented deferred subset (see
/// `docs/auth-messages/emir-auth106.md`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeWarningsRecord {
    /// Source file path (or other origin label).
    pub source_file: Option<String>,
    /// Stable identifier of the line within the source.
    pub record_id: Option<String>,
    /// Regulatory regime — always `Regime::Emir` for v1.
    pub regime: Regime,
    /// Reference date the warnings statistics cover.
    pub reporting_date: Option<NaiveDate>,
    /// LEI of the counterparty — `None` at the report-level aggregate
    /// (the per-counterparty `Wrnngs` breakdown is a deferred subset).
    pub counterparty_lei: Option<String>,
    /// Outstanding derivatives considered for missing-valuation.
    pub outstanding_derivatives: Option<i64>,
    /// Outstanding derivatives with no valuation reported.
    pub missing_valuation: Option<i64>,
    /// Outstanding derivatives whose valuation is outdated (>14 days).
    pub outdated_valuation: Option<i64>,
    /// Outstanding derivatives considered for missing-margin-info.
    pub outstanding_derivatives_margin: Option<i64>,
    /// Outstanding derivatives with no margin information reported.
    pub missing_margin_info: Option<i64>,
    /// Outstanding derivatives whose margin information is outdated.
    pub outdated_margin_info: Option<i64>,
    /// Derivatives reported (action NEWT/POSC/MODI/CORR) considered for
    /// the abnormal-values (notional outlier) check.
    pub derivatives_reported: Option<i64>,
    /// Derivatives reported whose notional is an abnormal outlier.
    pub abnormal_values: Option<i64>,
    /// Derived: `missing_valuation / outstanding_derivatives`.
    pub missing_valuation_rate: Option<Decimal>,
    /// Derived: `outdated_valuation / outstanding_derivatives`.
    pub outdated_valuation_rate: Option<Decimal>,
    /// Derived: `missing_margin_info / outstanding_derivatives_margin`.
    pub missing_margin_rate: Option<Decimal>,
    /// Derived: `outdated_margin_info / outstanding_derivatives_margin`.
    pub outdated_margin_rate: Option<Decimal>,
    /// Derived: `abnormal_values / derivatives_reported`.
    pub abnormal_values_rate: Option<Decimal>,
    /// Catch-all of XML leaves that were not promoted to typed fields.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub raw_fields: BTreeMap<String, String>,
}

impl Default for TradeWarningsRecord {
    fn default() -> Self {
        Self {
            source_file: None,
            record_id: None,
            regime: Regime::Emir,
            reporting_date: None,
            counterparty_lei: None,
            outstanding_derivatives: None,
            missing_valuation: None,
            outdated_valuation: None,
            outstanding_derivatives_margin: None,
            missing_margin_info: None,
            outdated_margin_info: None,
            derivatives_reported: None,
            abnormal_values: None,
            missing_valuation_rate: None,
            outdated_valuation_rate: None,
            missing_margin_rate: None,
            outdated_margin_rate: None,
            abnormal_values_rate: None,
            raw_fields: BTreeMap::new(),
        }
    }
}

/// One transaction from an SFTR Missing Collateral Request
/// (ISO 20022 `auth.083`,
/// `SecuritiesFinancingReportingMissingCollateralRequestV02`). The TR
/// asks the firm to provide the missing collateral for this SFT — one
/// record per `TxId`. See `docs/auth-messages/sftr-auth083.md`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissingCollateralRecord {
    /// Source file path (or other origin label).
    pub source_file: Option<String>,
    /// Stable identifier of the line within the source.
    pub record_id: Option<String>,
    /// Regulatory regime — always `Regime::Sftr`.
    pub regime: Regime,
    /// UTI of the SFT (`UnqTradIdr`) — optional in the message.
    pub uti: Option<String>,
    /// Reporting counterparty LEI (`RptgCtrPty/LEI`).
    pub reporting_counterparty: Option<String>,
    /// Other counterparty — `OthrCtrPty/Lgl/LEI` or, for a natural
    /// person, `OthrCtrPty/Ntrl/Id/Id`.
    pub other_counterparty: Option<String>,
    /// Master agreement type code (`MstrAgrmt/Tp/Tp`).
    pub master_agreement_type: Option<String>,
    /// Master agreement version (`MstrAgrmt/Vrsn`).
    pub master_agreement_version: Option<String>,
    /// Catch-all of XML leaves that were not promoted to typed fields.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub raw_fields: BTreeMap<String, String>,
}

impl Default for MissingCollateralRecord {
    fn default() -> Self {
        Self {
            source_file: None,
            record_id: None,
            regime: Regime::Sftr,
            uti: None,
            reporting_counterparty: None,
            other_counterparty: None,
            master_agreement_type: None,
            master_agreement_version: None,
            raw_fields: BTreeMap::new(),
        }
    }
}

/// Post-TR rejection profile loaded from `rejection_profile.yml`
/// (the YAML emitted by `opendqi feedback analytics`). Used to drive
/// the `EMIR.PSC.*` pre-submission check family so that historical
/// rejection patterns observed in TR feedback can flag risky records
/// before the firm re-submits them.
///
/// The YAML file wraps the payload under a `profile:` key — use
/// [`RejectionProfileFile`] when reading from disk.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RejectionProfile {
    /// Timestamp the profile was generated, ISO 8601.
    #[serde(default)]
    pub generated_at: Option<DateTime<Utc>>,
    /// Total feedback rows the analytics ran over.
    #[serde(default)]
    pub total_feedbacks: u64,
    /// Top rejection causes (sorted by descending count).
    #[serde(default)]
    pub top_causes: Vec<RejectionCause>,
    /// UTIs whose rejection count met the analytics threshold.
    #[serde(default)]
    pub repeated_rejected_utis: Vec<RepeatedRejection>,
}

/// One row in [`RejectionProfile::top_causes`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RejectionCause {
    /// TR-side reason code (e.g. `VAL01`).
    pub reason_code: String,
    /// Number of feedback rows carrying this reason.
    pub count: u64,
    /// Canonical OpenDQI check ID this reason maps to when known,
    /// e.g. `EMIR.COMP.UTI_MISSING`. Free-form when no mapping exists.
    #[serde(default)]
    pub suggested_check: Option<String>,
}

/// One row in [`RejectionProfile::repeated_rejected_utis`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RepeatedRejection {
    /// UTI rejected ≥ threshold times in the analytics window.
    pub uti: String,
    /// Number of distinct rejection events.
    pub count: u64,
}

/// On-disk wrapper for [`RejectionProfile`] — the YAML emitted by
/// `opendqi feedback analytics` nests the payload under `profile:`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RejectionProfileFile {
    /// The wrapped profile.
    pub profile: RejectionProfile,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evidence_item_serde_round_trip() {
        let e = EvidenceItem {
            field: "uti".into(),
            before: Some("OLD".into()),
            after: Some("NEW".into()),
            source_line: Some(42),
        };
        let j = serde_json::to_string(&e).unwrap();
        let back: EvidenceItem = serde_json::from_str(&j).unwrap();
        assert_eq!(e, back);
    }

    #[test]
    fn dq_issue_evidence_default_round_trip() {
        // Backward-compat: a JSON payload without `evidence` deserialises
        // with an empty vec (serde(default)).
        let json = r#"{"check_id":"X","regime":"emir","severity":"high","dimension":"completeness","record_id":null,"uti":null,"field":null,"value":null,"message":"m","source_file":null}"#;
        let issue: DqIssue = serde_json::from_str(json).unwrap();
        assert!(issue.evidence.is_empty());
    }

    #[test]
    fn feedback_validation_rules_serde() {
        // Empty list ⇒ key omitted (skip_serializing_if): existing
        // golden outputs stay byte-identical.
        let empty = FeedbackRecord::default();
        let j = serde_json::to_string(&empty).unwrap();
        assert!(!j.contains("validation_rule_codes"));

        // Legacy JSON without the field ⇒ deserialises to an empty vec.
        let legacy = r#"{"source_file":null,"record_id":null,"regime":"emir","feedback_type":"rejected","uti":"U1","reason_code":"VR-1","reason_description":null,"reported_field":null,"feedback_timestamp":null}"#;
        let r: FeedbackRecord = serde_json::from_str(legacy).unwrap();
        assert!(r.validation_rule_codes.is_empty());

        // Populated list round-trips.
        let full = FeedbackRecord {
            validation_rule_codes: vec!["VR-1".into(), "VR-2".into()],
            ..Default::default()
        };
        let back: FeedbackRecord =
            serde_json::from_str(&serde_json::to_string(&full).unwrap()).unwrap();
        assert_eq!(back.validation_rule_codes, vec!["VR-1", "VR-2"]);
    }
}
