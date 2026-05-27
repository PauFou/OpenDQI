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
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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

/// One line from an SFTR Margin Data Transaction State Report
/// (auth.085) — the TR's latest view of the margins exchanged
/// for one **CCP-cleared** collateral portfolio.
///
/// Scope (per ESMA XSD `auth.085.001.02_ESMAUG_1.1.0` =
/// `SecuritiesFinancingReportingMarginDataTransactionStateReportV02`):
///
/// > "latest state of the margins exchanged in relation to the
/// > CCP-cleared securities financing transactions"
///
/// **Important difference vs EMIR's `MarginStateRecord`
/// (auth.109)**:
///
/// - SFTR margins are reported at **portfolio level** (indexed
///   by `collateral_portfolio_code`), **not at trade/UTI
///   level** — `auth.085` has no `UnqTradIdr` at the per-record
///   element type (`CollateralMarginNew10__1`).
/// - SFTR carries **6 amounts** (no pre/post-haircut split):
///   `InitlMrgnPstd` / `VartnMrgnPstd` / `XcssCollPstd` +
///   `InitlMrgnRcvd` / `VartnMrgnRcvd` / `XcssCollRcvd`.
///   EMIR carries 4 (`initial_margin_posted_current`,
///   `initial_margin_collected_current`,
///   `variation_margin_posted_current`,
///   `variation_margin_collected_current`) without the
///   `XcssColl*` excess-collateral notion which is
///   SFTR-specific.
/// - SFTR has no `collateralization_category` (`FCOL` /
///   `PCOL` / `UCOL` / `OCOL`), no `collateral_market_value`,
///   no `haircut_applied` at MSR level (those live in the
///   transaction state report `auth.079` projected onto
///   `SftrTrStateRecord`).
///
/// Added in v0.17 alongside the parser
/// [`crate::dq::dqi::sftr_pack`] gains an `msr` input slot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SftrMarginStateRecord {
    /// Source file path (or other origin label).
    pub source_file: Option<String>,
    /// Stable identifier of the line within the source.
    /// Maps to `TechRcrdId` (Max140Text).
    pub record_id: Option<String>,
    /// Regulatory regime — always `Regime::Sftr` for this record.
    pub regime: Regime,
    /// Snapshot timestamp from `RptgDtTm` (the report's
    /// reporting date-time).
    pub state_as_of: Option<DateTime<Utc>>,
    /// `EvtDt` — date on which the reportable event captured
    /// by this margin state record took place.
    pub event_date: Option<NaiveDate>,
    /// LEI of the reporting counterparty
    /// (`CtrPty/RptgCtrPty/LEI` or equivalent).
    pub reporting_counterparty: Option<String>,
    /// LEI of the other counterparty (or natural person id)
    /// (`CtrPty/OthrCtrPty/Id/Lgl/LEI` or `Ntrl/Id/Id`).
    pub other_counterparty: Option<String>,
    /// `CollPrtflId` — unique and unambiguous identification
    /// of the collateral portfolio. **Required** by the XSD;
    /// Option-wrapped only to keep the struct uniformly
    /// constructible / serialisable.
    pub collateral_portfolio_code: Option<String>,
    /// Initial margin posted by the reporting counterparty to
    /// the other counterparty
    /// (`PstdMrgnOrColl/InitlMrgnPstd/Amt`).
    pub initial_margin_posted: Option<Decimal>,
    /// Variation margin posted, including cash settled
    /// (`PstdMrgnOrColl/VartnMrgnPstd/Amt`).
    pub variation_margin_posted: Option<Decimal>,
    /// Excess collateral posted (in excess of the required
    /// collateral) (`PstdMrgnOrColl/XcssCollPstd/Amt`).
    /// SFTR-specific (no EMIR equivalent).
    pub excess_collateral_posted: Option<Decimal>,
    /// Initial margin received from the other counterparty
    /// (`RcvdMrgnOrColl/InitlMrgnRcvd/Amt`).
    pub initial_margin_received: Option<Decimal>,
    /// Variation margin received, including cash settled
    /// (`RcvdMrgnOrColl/VartnMrgnRcvd/Amt`).
    pub variation_margin_received: Option<Decimal>,
    /// Excess collateral received (in excess of the required
    /// collateral) (`RcvdMrgnOrColl/XcssCollRcvd/Amt`).
    /// SFTR-specific (no EMIR equivalent).
    pub excess_collateral_received: Option<Decimal>,
    /// ISO 4217 currency, shared by all 6 amounts (promoted
    /// from the per-amount `@Ccy` attribute — the XSD allows
    /// them to differ but in practice a portfolio has a single
    /// margining currency).
    pub margin_currency: Option<String>,
    /// Action type from `CtrctMod/ActnTp` (NEWT / MODI /
    /// VALU / MARU / CORR / ETRM / POSC / OTHR).
    pub action_type: Option<String>,
    /// Catch-all of XML leaves not promoted to typed fields.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub raw_fields: BTreeMap<String, String>,
}

impl Default for SftrMarginStateRecord {
    fn default() -> Self {
        Self {
            source_file: None,
            record_id: None,
            regime: Regime::Sftr,
            state_as_of: None,
            event_date: None,
            reporting_counterparty: None,
            other_counterparty: None,
            collateral_portfolio_code: None,
            initial_margin_posted: None,
            variation_margin_posted: None,
            excess_collateral_posted: None,
            initial_margin_received: None,
            variation_margin_received: None,
            excess_collateral_received: None,
            margin_currency: None,
            action_type: None,
            raw_fields: BTreeMap::new(),
        }
    }
}

/// One line from an SFTR Transaction Margin Data Report (MAR)
/// (auth.070) — the per-event activity history of margins
/// exchanged for one **CCP-cleared** collateral portfolio.
///
/// Scope (per ESMA XSD `auth.070.001.02_ESMAUG_1.1.0` =
/// `SecuritiesFinancingReportingTransactionMarginDataReportV02`):
///
/// > "the margins exchanged in relation to the CCP-cleared
/// > securities financing transactions"
///
/// **Difference vs [`SftrMarginStateRecord`] (auth.085)** :
///
/// - auth.085 is the **state** snapshot (latest values per
///   portfolio) ; auth.070 is the **activity** event stream
///   (one record per posting / correction / error / margin
///   update). Same 6-amount data shape.
/// - The per-record element is `Rpt` (one of 4 wrapper
///   choices : `New` / `Err` / `Crrctn` / `TradUpd`). The
///   wrapper element name itself encodes the action type —
///   there's no separate `ActnTp` element under
///   `CtrctMod`. The parser projects the wrapper name onto
///   `action_type` as `"NEWT"` (for `New`), `"ERRT"`
///   (`Err`), `"CORR"` (`Crrctn`), `"TRDU"` (`TradUpd`).
/// - The `Err` wrapper carries no amounts and no `EvtDt` —
///   it's a pure retraction signal ; the corresponding record
///   has the 6 amount fields at `None` and `event_date` at
///   `None`.
/// - `event_date` (`EvtDt`) is semantically meaningful here
///   (the moment the activity took place, useful for
///   sliding-window analysis) ; on auth.085 it was the
///   snapshot reference date.
///
/// Mirror of EMIR's [`MarginActivityRecord`] (auth.108) but
/// adapted to the SFTR data shape (portfolio-level, 6 amounts
/// without pre/post-haircut split, `XcssColl*` SFTR-specific).
///
/// Added in v0.18 alongside the new `auth.070` parser
/// (`crates/opendqi-xml/src/sftr_margin_activity.rs`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SftrMarginActivityRecord {
    /// Source file path (or other origin label).
    pub source_file: Option<String>,
    /// Stable identifier of the line within the source.
    /// Maps to `TechRcrdId` (Max140Text).
    pub record_id: Option<String>,
    /// Regulatory regime — always `Regime::Sftr` for this record.
    pub regime: Regime,
    /// Report submission timestamp from `RptgDtTm`.
    pub state_as_of: Option<DateTime<Utc>>,
    /// `EvtDt` — date on which the reportable margin event
    /// (posting / correction / update) took place. None on
    /// `Err` (retraction) wrappers per the XSD.
    pub event_date: Option<NaiveDate>,
    /// LEI of the reporting counterparty
    /// (`CtrPty/RptgCtrPty/.../LEI`).
    pub reporting_counterparty: Option<String>,
    /// LEI of the other counterparty (or natural person id)
    /// (`CtrPty/OthrCtrPty/.../{Lgl/LEI | Ntrl/Id}`).
    pub other_counterparty: Option<String>,
    /// `CollPrtflId` — unique and unambiguous identification
    /// of the collateral portfolio. **Required** by the XSD ;
    /// Option-wrapped only to keep the struct uniformly
    /// constructible / serialisable.
    pub collateral_portfolio_code: Option<String>,
    /// Initial margin posted by the reporting counterparty to
    /// the other counterparty
    /// (`PstdMrgnOrColl/InitlMrgnPstd/Amt`). None on `Err`
    /// wrappers.
    pub initial_margin_posted: Option<Decimal>,
    /// Variation margin posted, including cash settled
    /// (`PstdMrgnOrColl/VartnMrgnPstd/Amt`).
    pub variation_margin_posted: Option<Decimal>,
    /// Excess collateral posted (in excess of the required
    /// collateral) (`PstdMrgnOrColl/XcssCollPstd/Amt`).
    /// SFTR-specific (no EMIR equivalent).
    pub excess_collateral_posted: Option<Decimal>,
    /// Initial margin received from the other counterparty
    /// (`RcvdMrgnOrColl/InitlMrgnRcvd/Amt`).
    pub initial_margin_received: Option<Decimal>,
    /// Variation margin received, including cash settled
    /// (`RcvdMrgnOrColl/VartnMrgnRcvd/Amt`).
    pub variation_margin_received: Option<Decimal>,
    /// Excess collateral received (in excess of the required
    /// collateral) (`RcvdMrgnOrColl/XcssCollRcvd/Amt`).
    /// SFTR-specific (no EMIR equivalent).
    pub excess_collateral_received: Option<Decimal>,
    /// ISO 4217 currency, shared by all 6 amounts (promoted
    /// from the first observed per-amount `@Ccy` attribute).
    pub margin_currency: Option<String>,
    /// Action type derived from the wrapper element name in
    /// `TradData/Rpt/<wrapper>` :
    /// `"NEWT"` (`New`), `"ERRT"` (`Err`), `"CORR"`
    /// (`Crrctn`), `"TRDU"` (`TradUpd`). Required by the XSD
    /// (every record is one of these 4 choices) ;
    /// Option-wrapped here for uniformity with the other SFTR
    /// records.
    pub action_type: Option<String>,
    /// Catch-all of XML leaves not promoted to typed fields.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub raw_fields: BTreeMap<String, String>,
}

impl Default for SftrMarginActivityRecord {
    fn default() -> Self {
        Self {
            source_file: None,
            record_id: None,
            regime: Regime::Sftr,
            state_as_of: None,
            event_date: None,
            reporting_counterparty: None,
            other_counterparty: None,
            collateral_portfolio_code: None,
            initial_margin_posted: None,
            variation_margin_posted: None,
            excess_collateral_posted: None,
            initial_margin_received: None,
            variation_margin_received: None,
            excess_collateral_received: None,
            margin_currency: None,
            action_type: None,
            raw_fields: BTreeMap::new(),
        }
    }
}

/// One line from an SFTR Reused Collateral Data Report (ISO 20022
/// `auth.071`) — the firm-side report of collateral reused or
/// reinvested. Event-driven, mirrors `SftrMarginActivityRecord`
/// in spirit but scoped to reuse events on the collateral the
/// firm received and then re-pledged or reinvested.
///
/// Distinctive vs `auth.070`:
/// - No `OthrCtrPty` — auth.071 is firm-portfolio-level, not
///   bilateral. The 3 entities captured are RptSubmitgNtty,
///   RptgCtrPty, NttyRspnsblForRpt (we promote only the first
///   two onto typed fields; the third lives in `raw_fields`).
/// - No `CollPrtflId` — records are keyed by submitter + event
///   day + (ISIN | cash currency).
/// - Reuse value is captured as a single aggregate amount (sum
///   of all `Scty/ReuseVal/{Actl|Estmtd}` entries observed) plus
///   a shared currency promoted from the first `@Ccy`. The per-
///   ISIN breakdown stays in `raw_fields`.
/// - Cash reuse exposes a single `cash_reinvestment_rate` (the
///   `CashReuseData1/CshRinvstmtRate` decimal percentage).
/// - The action_type is encoded in the wrapper element name
///   (mirror of auth.070 pattern): `New` → `NEWT`, `Err` →
///   `ERRT`, `Crrctn` → `CORR`, `CollReuseUpd` → `CRUD`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SftrReuseActivityRecord {
    /// Source file path (or other origin label).
    pub source_file: Option<String>,
    /// Stable identifier of the line within the source.
    /// Maps to `TechRcrdId` (Max140Text). Optional in `New` /
    /// required in `Err` per the XSD ; we treat it uniformly
    /// as Option.
    pub record_id: Option<String>,
    /// Regulatory regime — always `Regime::Sftr` for this record.
    pub regime: Regime,
    /// Report submission timestamp from `RptgDtTm`.
    pub state_as_of: Option<DateTime<Utc>>,
    /// `EvtDay` — date on which the reuse event took place.
    /// Absent on `Err` (retraction) wrappers per the XSD.
    pub event_day: Option<NaiveDate>,
    /// LEI of the reporting counterparty
    /// (`CtrPty/RptgCtrPty/.../LEI`).
    pub reporting_counterparty: Option<String>,
    /// LEI of the report-submitting entity, when distinct from
    /// the reporting counterparty (delegated reporting).
    /// (`CtrPty/RptSubmitgNtty/.../LEI`).
    pub report_submitting_entity: Option<String>,
    /// Action type derived from the wrapper element name in
    /// `TradData/Rpt/<wrapper>` :
    /// `"NEWT"` (`New`), `"ERRT"` (`Err`), `"CORR"`
    /// (`Crrctn`), `"CRUD"` (`CollReuseUpd`).
    pub action_type: Option<String>,
    /// Aggregate reused-value across every `CollCmpnt/Scty[]`
    /// entry observed — sum of all `Scty/ReuseVal/Actl` and
    /// `Scty/ReuseVal/Estmtd` amounts. Absent on `Err` wrappers
    /// and on cash-only reuse records.
    pub total_reuse_value: Option<Decimal>,
    /// ISO 4217 currency shared by all reuse amounts (promoted
    /// from the first observed per-amount `@Ccy` attribute).
    pub reuse_currency: Option<String>,
    /// Average interest rate received on cash collateral
    /// reinvestment (`CollCmpnt/Csh/CshRinvstmtRate`,
    /// `PercentageRate`). Absent when no cash reinvestment is
    /// reported.
    pub cash_reinvestment_rate: Option<Decimal>,
    /// Catch-all of XML leaves not promoted to typed fields :
    /// per-ISIN breakdown, individual `FndgSrc/Tp` and
    /// `FndgSrc/MktVal` entries, `NttyRspnsblForRpt/LEI`, etc.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub raw_fields: BTreeMap<String, String>,
}

impl Default for SftrReuseActivityRecord {
    fn default() -> Self {
        Self {
            source_file: None,
            record_id: None,
            regime: Regime::Sftr,
            state_as_of: None,
            event_day: None,
            reporting_counterparty: None,
            report_submitting_entity: None,
            action_type: None,
            total_reuse_value: None,
            reuse_currency: None,
            cash_reinvestment_rate: None,
            raw_fields: BTreeMap::new(),
        }
    }
}

/// One line from an SFTR Reused Collateral Data Transaction
/// State Report (ISO 20022 `auth.086`) — the TR-side snapshot
/// of the latest reuse / reinvestment state for the firm's
/// portfolios.
///
/// Structurally `auth.086` is the **state sister** of `auth.071`
/// (the activity log) : same content (CollCmpnt/Scty/Csh +
/// FndgSrc) but packed into the state-shaped `Stat[]` envelope
/// of `auth.085` (single `ReuseDataReportCorrection15__1` per
/// `Stat`, with a `CtrctMod/ActnTp` leaf — typically `REUU` —
/// instead of the 4-way action wrapper).
///
/// Field set is intentionally identical to
/// [`SftrReuseActivityRecord`] (12 fields). The semantic
/// difference :
/// - `auth.071` records are **events** (every NEWT/CORR/CRUD
///   wrapper is a discrete reuse event), produced by the firm.
/// - `auth.086` records are **state snapshots** (each `Stat` is
///   the current latest reuse state for a portfolio + event
///   day), produced by the TR.
///
/// The `auth.086` XSD has no `OthrCtrPty` (firm-portfolio-level,
/// not bilateral) and no UTI cross-reference. Records are keyed
/// intrinsically by submitter + event day + ISIN.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SftrReuseStateRecord {
    /// Source file path (or other origin label).
    pub source_file: Option<String>,
    /// Stable identifier of the line within the source.
    /// Maps to `TechRcrdId` (Max140Text).
    pub record_id: Option<String>,
    /// Regulatory regime — always `Regime::Sftr` for this record.
    pub regime: Regime,
    /// Report submission timestamp from `RptgDtTm`.
    pub state_as_of: Option<DateTime<Utc>>,
    /// `EvtDay` — date on which the reportable reuse event for
    /// which this snapshot is the latest state took place.
    pub event_day: Option<NaiveDate>,
    /// LEI of the reporting counterparty
    /// (`CtrPty/RptgCtrPty/.../LEI`).
    pub reporting_counterparty: Option<String>,
    /// LEI of the report-submitting entity, when distinct from
    /// the reporting counterparty (delegated reporting).
    /// (`CtrPty/RptSubmitgNtty/.../LEI`).
    pub report_submitting_entity: Option<String>,
    /// Action type from `CtrctMod/ActnTp`. Typically `"REUU"`
    /// (CollateralReuseUpdate) per the
    /// `TransactionOperationType6Code` enum. Option-wrapped so
    /// records constructed without the leaf stay representable.
    pub action_type: Option<String>,
    /// Aggregate latest-state reuse value across every
    /// `CollCmpnt/Scty[]` entry observed — sum of all
    /// `Scty/ReuseVal/Actl` and `Scty/ReuseVal/Estmtd` amounts.
    /// Absent on cash-only state records.
    pub total_reuse_value: Option<Decimal>,
    /// ISO 4217 currency shared by all reuse amounts (promoted
    /// from the first observed per-amount `@Ccy` attribute).
    pub reuse_currency: Option<String>,
    /// Latest-state average interest rate received on cash
    /// collateral reinvestment
    /// (`CollCmpnt/Csh/CshRinvstmtRate`, `PercentageRate`).
    pub cash_reinvestment_rate: Option<Decimal>,
    /// Catch-all of XML leaves not promoted to typed fields :
    /// per-ISIN breakdown, individual `FndgSrc/Tp` and
    /// `FndgSrc/MktVal` entries, `NttyRspnsblForRpt/LEI`, etc.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub raw_fields: BTreeMap<String, String>,
}

impl Default for SftrReuseStateRecord {
    fn default() -> Self {
        Self {
            source_file: None,
            record_id: None,
            regime: Regime::Sftr,
            state_as_of: None,
            event_day: None,
            reporting_counterparty: None,
            report_submitting_entity: None,
            action_type: None,
            total_reuse_value: None,
            reuse_currency: None,
            cash_reinvestment_rate: None,
            raw_fields: BTreeMap::new(),
        }
    }
}

/// One line from an SFTR Transaction Status Advice (ISO 20022
/// `auth.084`) — the TR-side aggregate rejection statistics
/// message identifying which transaction reports were rejected
/// and why.
///
/// Unlike the per-trade SFTR messages, `auth.084` is a
/// **statistics** message: one record per file carrying
/// aggregate counts (total reports, accepted, rejected) plus a
/// per-error-code breakdown.
///
/// Plan pivot honesty (v0.18 D): the v0.18 plan originally
/// scoped Phase D for `auth.078` (Pairing Request), but XSD
/// verification against the actual ESMA SFTR bundle showed
/// `auth.078` is not in the published message set — pairing
/// semantics are carried by `auth.080` (Reconciliation Status
/// Advice, already covered). Pivoted Phase D to `auth.084`
/// (Transaction Status Advice, the actual SFTR rejection-
/// feedback message) — closes a real gap in our coverage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SftrTrStatusAdviceRecord {
    /// Source file path (or other origin label).
    pub source_file: Option<String>,
    /// Stable identifier of the line within the source.
    /// `auth.084` has no `TechRcrdId` element so the parser
    /// synthesises one from `source_file` + record index.
    pub record_id: Option<String>,
    /// Regulatory regime — always `Regime::Sftr` for this record.
    pub regime: Regime,
    /// `RptSttstcs/TtlNbOfRpts` — total number of reports
    /// sent or received in the reporting period.
    pub total_reports: Option<u64>,
    /// `RptSttstcs/TtlNbOfRptsAccptd` — number of reports
    /// accepted by the TR.
    pub total_reports_accepted: Option<u64>,
    /// `RptSttstcs/TtlNbOfRptsRjctd` — number of reports
    /// rejected by the TR.
    pub total_reports_rejected: Option<u64>,
    /// Per-error-code breakdown of rejected reports
    /// (`RptSttstcs/NbOfRptsRjctdPerErr[]` keyed by validation
    /// rule code). Empty when no per-error rows are reported.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub rejected_reports_per_error: BTreeMap<String, u64>,
    /// Catch-all of XML leaves not promoted to typed fields
    /// (transaction-level statistics under `TxSttstcs`,
    /// reporting-period metadata, etc.).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub raw_fields: BTreeMap<String, String>,
}

impl Default for SftrTrStatusAdviceRecord {
    fn default() -> Self {
        Self {
            source_file: None,
            record_id: None,
            regime: Regime::Sftr,
            total_reports: None,
            total_reports_accepted: None,
            total_reports_rejected: None,
            rejected_reports_per_error: BTreeMap::new(),
            raw_fields: BTreeMap::new(),
        }
    }
}

/// One position-set row from an EMIR Derivatives Trade Position
/// Set Report (ISO 20022 `auth.090`) — the TR-side aggregated
/// exposures between a pair of counterparties.
///
/// Unlike per-trade `auth.030`/`auth.107`, `auth.090` is a
/// **statistics** message: each `PosSet` (or `CcyPosSet`,
/// `CollPosSet`, `CcyCollPosSet`) is an aggregate over many
/// outstanding derivatives sharing the same dimensions (CP,
/// asset class, contract type, value currency, …) and carries
/// a metrics block (notional, MtM value, collateral, …).
///
/// We model the most DQ-actionable subset of the 5400-line XSD
/// — 14 typed fields covering the dimensions and the headline
/// metrics. The full per-leg / per-event detail (Direction,
/// CommonTradeDataReport, EnergyCommodity*, …) is captured
/// into `raw_fields` for downstream inspection.
///
/// The 4 position-set kinds are discriminated via
/// `position_set_kind` (`"PosSet"`, `"CcyPosSet"`,
/// `"CollPosSet"`, `"CcyCollPosSet"`) so DQI computers can
/// scope by kind.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmirPositionSetRecord {
    /// Source file path (or other origin label).
    pub source_file: Option<String>,
    /// Synthetic stable identifier (`<source>#<kind>-<index>`).
    /// auth.090 has no per-record id at the XSD level; the
    /// parser stamps one based on file + kind + 1-based index.
    pub record_id: Option<String>,
    /// Regulatory regime — always `Regime::Emir` for v1.
    pub regime: Regime,
    /// `Rpt/RefDt` — reference date for statistics collection.
    /// Shared across every record in the same file.
    pub reference_date: Option<NaiveDate>,
    /// Which of the 4 aggregated-position kinds the record
    /// represents : `"PosSet"`, `"CcyPosSet"`, `"CollPosSet"`,
    /// `"CcyCollPosSet"`.
    pub position_set_kind: Option<String>,
    /// LEI of the reporting counterparty
    /// (`Dmnsns/CtrPtyId/.../RptgCtrPty/.../LEI`).
    pub reporting_counterparty: Option<String>,
    /// LEI of the other counterparty
    /// (`Dmnsns/CtrPtyId/.../OthrCtrPty/.../LEI`).
    pub other_counterparty: Option<String>,
    /// Asset class enum (`Dmnsns/AsstClss`, `ProductType4Code`):
    /// `CRDT`, `CURR`, `EQUI`, `INTR`, `COMM`, etc.
    pub asset_class: Option<String>,
    /// Contract type enum (`Dmnsns/CtrctTp`,
    /// `FinancialInstrumentContractType2Code`): `OPTN`, `FUTR`,
    /// `FRWD`, `SWAP`, `CFDS`, etc.
    pub contract_type: Option<String>,
    /// ISO 4217 valuation currency (`Dmnsns/ValCcy`).
    pub value_currency: Option<String>,
    /// Underlying instrument id (`Dmnsns/UndrlygInstrm/.../ISIN`)
    /// when reported as an ISIN.
    pub underlying_id: Option<String>,
    /// Aggregated notional metric in the value currency. The
    /// parser promotes the first observed amount in
    /// `Mtrcs/.../Amt` (XSD has several alternates depending
    /// on the metric type).
    pub notional: Option<Decimal>,
    /// Aggregated mark-to-market value
    /// (`Mtrcs/MtMVal/Amt` when present).
    pub mark_to_market_value: Option<Decimal>,
    /// Aggregated collateral value (CollPosSet kinds only).
    pub collateral_value: Option<Decimal>,
    /// Catch-all of XML leaves not promoted to typed fields.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub raw_fields: BTreeMap<String, String>,
}

impl Default for EmirPositionSetRecord {
    fn default() -> Self {
        Self {
            source_file: None,
            record_id: None,
            regime: Regime::Emir,
            reference_date: None,
            position_set_kind: None,
            reporting_counterparty: None,
            other_counterparty: None,
            asset_class: None,
            contract_type: None,
            value_currency: None,
            underlying_id: None,
            notional: None,
            mark_to_market_value: None,
            collateral_value: None,
            raw_fields: BTreeMap::new(),
        }
    }
}

/// One query record from an EMIR Derivatives Trade Report Query
/// (ISO 20022 `auth.029`) — a firm-side request the firm sends to
/// the TR to retrieve its own derivatives data.
///
/// Unlike the report messages OpenDQI normally ingests (TR → firm
/// flows : auth.030, auth.107, auth.092, auth.106, …), auth.029
/// is a **request envelope** the firm sends *to* the TR. It carries
/// no derivatives payload itself — only the identifier of the
/// querying firm, the timestamp of the request, and the filter
/// criteria that scope which trades the TR should return.
///
/// We therefore model only the envelope-level fields here. There
/// is no business DQ signal beyond an `ENVELOPE_WELLFORMED`
/// sanity check verifying that a query carries the minimum
/// identifying information (query id + requesting LEI). See
/// [`crate::dq::emir_query`] for the check and
/// `docs/auth-messages/emir-auth029.md` for the rationale.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmirQueryRecord {
    /// Source file path (or other origin label).
    pub source_file: Option<String>,
    /// Synthetic stable identifier (`<source>#Qry-<index>`).
    /// auth.029 has no per-record id at the XSD level; the
    /// parser stamps one based on file + 1-based index.
    pub record_id: Option<String>,
    /// Regulatory regime — always `Regime::Emir`.
    pub regime: Regime,
    /// Query-level identifier reported by the firm (when present).
    /// Optional because the XSD allows queries to be sent without
    /// an explicit id, in which case the parser leaves this `None`
    /// and only `record_id` is populated.
    pub query_id: Option<String>,
    /// Timestamp the query was emitted by the firm
    /// (`MsgHdr/CreDtTm` or the request-time equivalent).
    pub query_timestamp: Option<DateTime<Utc>>,
    /// LEI of the entity issuing the query
    /// (`ReqstngPty/.../LEI` or equivalent), used as the
    /// minimum-identity field for the `ENVELOPE_WELLFORMED` check.
    pub requesting_lei: Option<String>,
    /// Free-form descriptions of the filter criteria that scope
    /// the query (date range, UTI list, counterparty filters, …).
    /// One string per filter element observed; format is left
    /// opaque because the XSD allows many alternates and the
    /// content is not used for DQ checks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filter_descriptions: Vec<String>,
    /// Catch-all of XML leaves not promoted to typed fields.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub raw_fields: BTreeMap<String, String>,
}

impl Default for EmirQueryRecord {
    fn default() -> Self {
        Self {
            source_file: None,
            record_id: None,
            regime: Regime::Emir,
            query_id: None,
            query_timestamp: None,
            requesting_lei: None,
            filter_descriptions: Vec::new(),
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

/// One per-counterparty line from the `Wrnngs` breakdown of an EMIR
/// Data-Quality Warnings Report (ISO 20022 `auth.106`). The TR breaks
/// the report-level statistics down per reporting counterparty; this
/// record carries that per-LEI view for one reference date (the three
/// `MssngValtn` / `MssngMrgnInf` / `AbnrmlVals` sub-reports for the
/// same LEI are merged). The deeper per-UTI `TxDtls` level is a
/// documented deferred subset (see `docs/auth-messages/emir-auth106.md`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarningsCounterpartyRecord {
    /// Source file path (or other origin label).
    pub source_file: Option<String>,
    /// Stable identifier of the line within the source.
    pub record_id: Option<String>,
    /// Regulatory regime — always `Regime::Emir` for v1.
    pub regime: Regime,
    /// Reference date the warnings statistics cover.
    pub reporting_date: Option<NaiveDate>,
    /// LEI of the counterparty this `Wrnngs` block is for.
    pub counterparty_lei: Option<String>,
    /// Outstanding derivatives considered for missing-valuation.
    pub outstanding_derivatives: Option<i64>,
    /// Outstanding derivatives with no valuation reported.
    pub missing_valuation: Option<i64>,
    /// Outstanding derivatives whose valuation is outdated.
    pub outdated_valuation: Option<i64>,
    /// Outstanding derivatives considered for missing-margin-info.
    pub outstanding_derivatives_margin: Option<i64>,
    /// Outstanding derivatives with no margin information reported.
    pub missing_margin_info: Option<i64>,
    /// Outstanding derivatives whose margin information is outdated.
    pub outdated_margin_info: Option<i64>,
    /// Derivatives reported considered for the abnormal-values check.
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

impl Default for WarningsCounterpartyRecord {
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

/// One transaction the TR explicitly flagged inside an EMIR
/// Data-Quality Warnings Report (ISO 20022 `auth.106`,
/// `Wrnngs/TxDtls`). Each record is one problematic SFT enumerated by
/// the TR under a counterparty for one reference date — operational,
/// not statistical. `warning_category` distinguishes which sub-report
/// flagged it (`MissingValuation` / `MissingMargin` / `AbnormalValue`);
/// the heterogeneous per-category context (valuation amount/timestamp,
/// collateral timestamp, notional, action/event metadata) is preserved
/// in `raw_fields`. See `docs/auth-messages/emir-auth106.md`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarningsTransactionRecord {
    /// Source file path (or other origin label).
    pub source_file: Option<String>,
    /// Stable identifier of the line within the source.
    pub record_id: Option<String>,
    /// Regulatory regime — always `Regime::Emir` for v1.
    pub regime: Regime,
    /// Reference date the warnings statistics cover.
    pub reporting_date: Option<NaiveDate>,
    /// LEI of the counterparty this `Wrnngs` block is for.
    pub counterparty_lei: Option<String>,
    /// UTI of the flagged transaction (`TxId/UnqIdr/UnqTxIdr`, else
    /// the proprietary `TxId/UnqIdr/Prtry/Id`).
    pub uti: Option<String>,
    /// Which sub-report flagged this transaction:
    /// `MissingValuation` | `MissingMargin` | `AbnormalValue`.
    pub warning_category: Option<String>,
    /// Other counterparty of the flagged trade
    /// (`TxId/OthrCtrPty/.../LEI`), when present.
    pub other_counterparty: Option<String>,
    /// Catch-all of the heterogeneous per-category `TxDtls` context
    /// leaves (valuation amount/currency/timestamp, collateral
    /// timestamp, notional amount, action/event metadata).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub raw_fields: BTreeMap<String, String>,
}

impl Default for WarningsTransactionRecord {
    fn default() -> Self {
        Self {
            source_file: None,
            record_id: None,
            regime: Regime::Emir,
            reporting_date: None,
            counterparty_lei: None,
            uti: None,
            warning_category: None,
            other_counterparty: None,
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

    #[test]
    fn sftr_margin_state_record_default_is_sftr_with_all_options_none() {
        // Default constructs into a Sftr-regime record with every
        // amount/currency/action field at None and an empty
        // `raw_fields`. Locks the v0.17 surface contract.
        let r = SftrMarginStateRecord::default();
        assert_eq!(r.regime, Regime::Sftr);
        assert!(r.state_as_of.is_none());
        assert!(r.event_date.is_none());
        assert!(r.reporting_counterparty.is_none());
        assert!(r.other_counterparty.is_none());
        assert!(r.collateral_portfolio_code.is_none());
        assert!(r.initial_margin_posted.is_none());
        assert!(r.variation_margin_posted.is_none());
        assert!(r.excess_collateral_posted.is_none());
        assert!(r.initial_margin_received.is_none());
        assert!(r.variation_margin_received.is_none());
        assert!(r.excess_collateral_received.is_none());
        assert!(r.margin_currency.is_none());
        assert!(r.action_type.is_none());
        assert!(r.raw_fields.is_empty());
    }

    #[test]
    fn sftr_margin_state_record_serde_always_emits_amount_keys_when_none() {
        // Mirror the EMIR `MarginStateRecord` convention: the 6
        // amount fields + currency serialize as JSON `null` when
        // None (no `skip_serializing_if`). Keeps downstream Parquet
        // / CSV writers schema-stable — every MSR row has the same
        // column set whether populated or not. The only catch-all
        // field that IS skip-serialized is `raw_fields` (when
        // empty).
        let r = SftrMarginStateRecord::default();
        let j = serde_json::to_string(&r).unwrap();
        for key in [
            "initial_margin_posted",
            "variation_margin_posted",
            "excess_collateral_posted",
            "initial_margin_received",
            "variation_margin_received",
            "excess_collateral_received",
            "margin_currency",
            "reporting_counterparty",
            "other_counterparty",
            "collateral_portfolio_code",
            "action_type",
            "event_date",
            "state_as_of",
        ] {
            assert!(
                j.contains(&format!("\"{key}\":")),
                "default JSON must include key {key}: {j}"
            );
        }
        // raw_fields is skip-serialised when empty.
        assert!(!j.contains("raw_fields"));
    }

    #[test]
    fn sftr_margin_state_record_round_trip_populated() {
        // A fully-populated record (all 6 amounts + currency +
        // portfolio + counterparties + action_type) round-trips
        // through serde with byte-identical equality on every
        // field. Locks the v1.0 schema contract for the new
        // SFTR MSR layer.
        use rust_decimal::Decimal;
        use std::str::FromStr;
        let r = SftrMarginStateRecord {
            source_file: Some("auth085.xml".into()),
            record_id: Some("REC-1".into()),
            regime: Regime::Sftr,
            collateral_portfolio_code: Some("PORTFOLIO-1".into()),
            reporting_counterparty: Some("549300ABCDEFGH123456".into()),
            other_counterparty: Some("549300ZYXWVU987654ZZ".into()),
            initial_margin_posted: Some(Decimal::from_str("1000.50").unwrap()),
            variation_margin_posted: Some(Decimal::from_str("50.00").unwrap()),
            excess_collateral_posted: Some(Decimal::from_str("25.25").unwrap()),
            initial_margin_received: Some(Decimal::from_str("980.75").unwrap()),
            variation_margin_received: Some(Decimal::from_str("48.00").unwrap()),
            excess_collateral_received: Some(Decimal::from_str("20.10").unwrap()),
            margin_currency: Some("EUR".into()),
            action_type: Some("MARU".into()),
            ..SftrMarginStateRecord::default()
        };
        let j = serde_json::to_string(&r).unwrap();
        let back: SftrMarginStateRecord = serde_json::from_str(&j).unwrap();
        assert_eq!(back.source_file, r.source_file);
        assert_eq!(back.record_id, r.record_id);
        assert_eq!(back.regime, r.regime);
        assert_eq!(back.collateral_portfolio_code, r.collateral_portfolio_code);
        assert_eq!(back.reporting_counterparty, r.reporting_counterparty);
        assert_eq!(back.other_counterparty, r.other_counterparty);
        assert_eq!(back.initial_margin_posted, r.initial_margin_posted);
        assert_eq!(back.variation_margin_posted, r.variation_margin_posted);
        assert_eq!(back.excess_collateral_posted, r.excess_collateral_posted);
        assert_eq!(back.initial_margin_received, r.initial_margin_received);
        assert_eq!(back.variation_margin_received, r.variation_margin_received);
        assert_eq!(
            back.excess_collateral_received,
            r.excess_collateral_received
        );
        assert_eq!(back.margin_currency, r.margin_currency);
        assert_eq!(back.action_type, r.action_type);
    }

    #[test]
    fn sftr_margin_activity_record_default_is_sftr_with_all_options_none() {
        // v0.18 A1 — mirror of sftr_margin_state_record_default_is_*
        // for the new MAR record. Locks the v0.18 public surface
        // (16-field struct, regime=Sftr default, all fields None,
        // empty raw_fields).
        let r = SftrMarginActivityRecord::default();
        assert_eq!(r.regime, Regime::Sftr);
        assert!(r.state_as_of.is_none());
        assert!(r.event_date.is_none());
        assert!(r.reporting_counterparty.is_none());
        assert!(r.other_counterparty.is_none());
        assert!(r.collateral_portfolio_code.is_none());
        assert!(r.initial_margin_posted.is_none());
        assert!(r.variation_margin_posted.is_none());
        assert!(r.excess_collateral_posted.is_none());
        assert!(r.initial_margin_received.is_none());
        assert!(r.variation_margin_received.is_none());
        assert!(r.excess_collateral_received.is_none());
        assert!(r.margin_currency.is_none());
        assert!(r.action_type.is_none());
        assert!(r.raw_fields.is_empty());
    }

    #[test]
    fn sftr_margin_activity_record_serde_always_emits_metadata_keys_when_none() {
        // Mirror of sftr_margin_state_record_serde_always_emits_*.
        // Every Option<T> serialises as JSON null when None
        // (no skip_serializing_if) — keeps Parquet/CSV schemas
        // stable. The catch-all raw_fields IS skip-serialised
        // when empty.
        let r = SftrMarginActivityRecord::default();
        let j = serde_json::to_string(&r).unwrap();
        for key in [
            "initial_margin_posted",
            "variation_margin_posted",
            "excess_collateral_posted",
            "initial_margin_received",
            "variation_margin_received",
            "excess_collateral_received",
            "margin_currency",
            "reporting_counterparty",
            "other_counterparty",
            "collateral_portfolio_code",
            "action_type",
            "event_date",
            "state_as_of",
        ] {
            assert!(
                j.contains(&format!("\"{key}\":")),
                "default JSON must include key {key}: {j}"
            );
        }
        assert!(!j.contains("raw_fields"));
    }

    #[test]
    fn sftr_margin_activity_record_round_trip_populated() {
        // Full event payload (NEWT wrapper with all 6 amounts +
        // currency + portfolio + CPs) round-trips through serde
        // without loss. Locks the v0.18 schema contract for the
        // new MAR layer.
        use rust_decimal::Decimal;
        use std::str::FromStr;
        let r = SftrMarginActivityRecord {
            source_file: Some("auth070.xml".into()),
            record_id: Some("EVT-1".into()),
            regime: Regime::Sftr,
            collateral_portfolio_code: Some("PORTFOLIO-1".into()),
            reporting_counterparty: Some("549300ABCDEFGH123456".into()),
            other_counterparty: Some("549300ZYXWVU987654ZZ".into()),
            initial_margin_posted: Some(Decimal::from_str("1000.50").unwrap()),
            variation_margin_posted: Some(Decimal::from_str("50.00").unwrap()),
            excess_collateral_posted: Some(Decimal::from_str("25.25").unwrap()),
            initial_margin_received: Some(Decimal::from_str("980.75").unwrap()),
            variation_margin_received: Some(Decimal::from_str("48.00").unwrap()),
            excess_collateral_received: Some(Decimal::from_str("20.10").unwrap()),
            margin_currency: Some("EUR".into()),
            action_type: Some("NEWT".into()),
            ..SftrMarginActivityRecord::default()
        };
        let j = serde_json::to_string(&r).unwrap();
        let back: SftrMarginActivityRecord = serde_json::from_str(&j).unwrap();
        assert_eq!(back.source_file, r.source_file);
        assert_eq!(back.record_id, r.record_id);
        assert_eq!(back.regime, r.regime);
        assert_eq!(back.collateral_portfolio_code, r.collateral_portfolio_code);
        assert_eq!(back.reporting_counterparty, r.reporting_counterparty);
        assert_eq!(back.other_counterparty, r.other_counterparty);
        assert_eq!(back.initial_margin_posted, r.initial_margin_posted);
        assert_eq!(back.variation_margin_posted, r.variation_margin_posted);
        assert_eq!(back.excess_collateral_posted, r.excess_collateral_posted);
        assert_eq!(back.initial_margin_received, r.initial_margin_received);
        assert_eq!(back.variation_margin_received, r.variation_margin_received);
        assert_eq!(
            back.excess_collateral_received,
            r.excess_collateral_received
        );
        assert_eq!(back.margin_currency, r.margin_currency);
        assert_eq!(back.action_type, r.action_type);
    }

    // -- v0.18 B1: SftrReuseActivityRecord (auth.071) ----------

    #[test]
    fn sftr_reuse_activity_record_default_is_sftr_with_all_options_none() {
        let r = SftrReuseActivityRecord::default();
        assert_eq!(r.regime, Regime::Sftr);
        assert!(r.source_file.is_none());
        assert!(r.record_id.is_none());
        assert!(r.state_as_of.is_none());
        assert!(r.event_day.is_none());
        assert!(r.reporting_counterparty.is_none());
        assert!(r.report_submitting_entity.is_none());
        assert!(r.action_type.is_none());
        assert!(r.total_reuse_value.is_none());
        assert!(r.reuse_currency.is_none());
        assert!(r.cash_reinvestment_rate.is_none());
        assert!(r.raw_fields.is_empty());
    }

    #[test]
    fn sftr_reuse_activity_record_serde_always_emits_metadata_keys_when_none() {
        // Mirror of the v0.17 SftrMarginStateRecord contract test:
        // critical metadata fields stay serialised as `null` (not
        // skipped) so downstream consumers can rely on the key
        // appearing in every JSON record.
        let r = SftrReuseActivityRecord::default();
        let j = serde_json::to_string(&r).unwrap();
        for required_key in [
            "source_file",
            "record_id",
            "state_as_of",
            "event_day",
            "reporting_counterparty",
            "report_submitting_entity",
            "action_type",
            "total_reuse_value",
            "reuse_currency",
            "cash_reinvestment_rate",
        ] {
            assert!(
                j.contains(&format!("\"{required_key}\":null")),
                "key {required_key} should serialise as null on default, json was {j}"
            );
        }
        // raw_fields is skip-serialized when empty (matches the
        // shipped pattern on the 5 other SFTR record types).
        assert!(
            !j.contains("\"raw_fields\""),
            "raw_fields should be skipped when empty"
        );
    }

    #[test]
    fn sftr_reuse_activity_record_round_trip_populated() {
        // Full NEWT-wrapper payload: aggregate reuse value +
        // currency + cash reinvestment rate + both CP fields.
        // Locks the v0.18 schema contract for the new reuse layer.
        use rust_decimal::Decimal;
        use std::str::FromStr;
        let r = SftrReuseActivityRecord {
            source_file: Some("auth071.xml".into()),
            record_id: Some("REUSE-1".into()),
            regime: Regime::Sftr,
            reporting_counterparty: Some("549300ABCDEFGH123456".into()),
            report_submitting_entity: Some("549300SUBMITRPT00001".into()),
            total_reuse_value: Some(Decimal::from_str("12345.67").unwrap()),
            reuse_currency: Some("EUR".into()),
            cash_reinvestment_rate: Some(Decimal::from_str("0.0125").unwrap()),
            action_type: Some("NEWT".into()),
            ..SftrReuseActivityRecord::default()
        };
        let j = serde_json::to_string(&r).unwrap();
        let back: SftrReuseActivityRecord = serde_json::from_str(&j).unwrap();
        assert_eq!(back.source_file, r.source_file);
        assert_eq!(back.record_id, r.record_id);
        assert_eq!(back.regime, r.regime);
        assert_eq!(back.reporting_counterparty, r.reporting_counterparty);
        assert_eq!(back.report_submitting_entity, r.report_submitting_entity);
        assert_eq!(back.total_reuse_value, r.total_reuse_value);
        assert_eq!(back.reuse_currency, r.reuse_currency);
        assert_eq!(back.cash_reinvestment_rate, r.cash_reinvestment_rate);
        assert_eq!(back.action_type, r.action_type);
    }

    // -- v0.18 C1: SftrReuseStateRecord (auth.086) -------------

    #[test]
    fn sftr_reuse_state_record_default_is_sftr_with_all_options_none() {
        let r = SftrReuseStateRecord::default();
        assert_eq!(r.regime, Regime::Sftr);
        assert!(r.source_file.is_none());
        assert!(r.record_id.is_none());
        assert!(r.state_as_of.is_none());
        assert!(r.event_day.is_none());
        assert!(r.reporting_counterparty.is_none());
        assert!(r.report_submitting_entity.is_none());
        assert!(r.action_type.is_none());
        assert!(r.total_reuse_value.is_none());
        assert!(r.reuse_currency.is_none());
        assert!(r.cash_reinvestment_rate.is_none());
        assert!(r.raw_fields.is_empty());
    }

    #[test]
    fn sftr_reuse_state_record_serde_always_emits_metadata_keys_when_none() {
        let r = SftrReuseStateRecord::default();
        let j = serde_json::to_string(&r).unwrap();
        for required_key in [
            "source_file",
            "record_id",
            "state_as_of",
            "event_day",
            "reporting_counterparty",
            "report_submitting_entity",
            "action_type",
            "total_reuse_value",
            "reuse_currency",
            "cash_reinvestment_rate",
        ] {
            assert!(
                j.contains(&format!("\"{required_key}\":null")),
                "key {required_key} should serialise as null on default, json was {j}"
            );
        }
        assert!(
            !j.contains("\"raw_fields\""),
            "raw_fields should be skipped when empty"
        );
    }

    #[test]
    fn sftr_reuse_state_record_round_trip_populated() {
        use rust_decimal::Decimal;
        use std::str::FromStr;
        let r = SftrReuseStateRecord {
            source_file: Some("auth086.xml".into()),
            record_id: Some("REUSE-STATE-1".into()),
            regime: Regime::Sftr,
            reporting_counterparty: Some("549300ABCDEFGH123456".into()),
            report_submitting_entity: Some("549300SUBMITRPT00001".into()),
            total_reuse_value: Some(Decimal::from_str("98765.43").unwrap()),
            reuse_currency: Some("USD".into()),
            cash_reinvestment_rate: Some(Decimal::from_str("0.0250").unwrap()),
            action_type: Some("REUU".into()),
            ..SftrReuseStateRecord::default()
        };
        let j = serde_json::to_string(&r).unwrap();
        let back: SftrReuseStateRecord = serde_json::from_str(&j).unwrap();
        assert_eq!(back.source_file, r.source_file);
        assert_eq!(back.record_id, r.record_id);
        assert_eq!(back.regime, r.regime);
        assert_eq!(back.reporting_counterparty, r.reporting_counterparty);
        assert_eq!(back.report_submitting_entity, r.report_submitting_entity);
        assert_eq!(back.total_reuse_value, r.total_reuse_value);
        assert_eq!(back.reuse_currency, r.reuse_currency);
        assert_eq!(back.cash_reinvestment_rate, r.cash_reinvestment_rate);
        assert_eq!(back.action_type, r.action_type);
    }

    // -- v0.18 D1: SftrTrStatusAdviceRecord (auth.084) ---------

    #[test]
    fn sftr_tr_status_advice_record_default_is_sftr_with_all_options_none() {
        let r = SftrTrStatusAdviceRecord::default();
        assert_eq!(r.regime, Regime::Sftr);
        assert!(r.source_file.is_none());
        assert!(r.record_id.is_none());
        assert!(r.total_reports.is_none());
        assert!(r.total_reports_accepted.is_none());
        assert!(r.total_reports_rejected.is_none());
        assert!(r.rejected_reports_per_error.is_empty());
        assert!(r.raw_fields.is_empty());
    }

    #[test]
    fn sftr_tr_status_advice_record_serde_always_emits_metadata_keys_when_none() {
        let r = SftrTrStatusAdviceRecord::default();
        let j = serde_json::to_string(&r).unwrap();
        for required_key in [
            "source_file",
            "record_id",
            "total_reports",
            "total_reports_accepted",
            "total_reports_rejected",
        ] {
            assert!(
                j.contains(&format!("\"{required_key}\":null")),
                "key {required_key} should serialise as null on default, json was {j}"
            );
        }
        // Maps skip-serialize when empty.
        assert!(!j.contains("\"rejected_reports_per_error\""));
        assert!(!j.contains("\"raw_fields\""));
    }

    #[test]
    fn sftr_tr_status_advice_record_round_trip_populated() {
        let mut per_err = BTreeMap::new();
        per_err.insert("VR-001".into(), 12u64);
        per_err.insert("VR-002".into(), 3u64);
        let r = SftrTrStatusAdviceRecord {
            source_file: Some("auth084.xml".into()),
            record_id: Some("auth084.xml#rpt-1".into()),
            regime: Regime::Sftr,
            total_reports: Some(1000),
            total_reports_accepted: Some(985),
            total_reports_rejected: Some(15),
            rejected_reports_per_error: per_err.clone(),
            ..SftrTrStatusAdviceRecord::default()
        };
        let j = serde_json::to_string(&r).unwrap();
        let back: SftrTrStatusAdviceRecord = serde_json::from_str(&j).unwrap();
        assert_eq!(back.source_file, r.source_file);
        assert_eq!(back.record_id, r.record_id);
        assert_eq!(back.total_reports, Some(1000));
        assert_eq!(back.total_reports_accepted, Some(985));
        assert_eq!(back.total_reports_rejected, Some(15));
        assert_eq!(back.rejected_reports_per_error, per_err);
    }

    // -- v0.18 E1: EmirPositionSetRecord (auth.090) ------------

    #[test]
    fn emir_position_set_record_default_is_emir_with_all_options_none() {
        let r = EmirPositionSetRecord::default();
        assert_eq!(r.regime, Regime::Emir);
        assert!(r.source_file.is_none());
        assert!(r.record_id.is_none());
        assert!(r.reference_date.is_none());
        assert!(r.position_set_kind.is_none());
        assert!(r.reporting_counterparty.is_none());
        assert!(r.other_counterparty.is_none());
        assert!(r.asset_class.is_none());
        assert!(r.contract_type.is_none());
        assert!(r.value_currency.is_none());
        assert!(r.underlying_id.is_none());
        assert!(r.notional.is_none());
        assert!(r.mark_to_market_value.is_none());
        assert!(r.collateral_value.is_none());
        assert!(r.raw_fields.is_empty());
    }

    #[test]
    fn emir_position_set_record_serde_always_emits_metadata_keys_when_none() {
        let r = EmirPositionSetRecord::default();
        let j = serde_json::to_string(&r).unwrap();
        for required_key in [
            "source_file",
            "record_id",
            "reference_date",
            "position_set_kind",
            "reporting_counterparty",
            "other_counterparty",
            "asset_class",
            "contract_type",
            "value_currency",
            "underlying_id",
            "notional",
            "mark_to_market_value",
            "collateral_value",
        ] {
            assert!(
                j.contains(&format!("\"{required_key}\":null")),
                "key {required_key} should serialise as null on default, json was {j}"
            );
        }
        assert!(!j.contains("\"raw_fields\""));
    }

    #[test]
    fn emir_position_set_record_round_trip_populated() {
        use chrono::NaiveDate;
        use rust_decimal::Decimal;
        use std::str::FromStr;
        let r = EmirPositionSetRecord {
            source_file: Some("auth090.xml".into()),
            record_id: Some("auth090.xml#PosSet-1".into()),
            regime: Regime::Emir,
            reference_date: NaiveDate::from_ymd_opt(2026, 5, 21),
            position_set_kind: Some("PosSet".into()),
            reporting_counterparty: Some("549300ABCDEFGH123456".into()),
            other_counterparty: Some("549300ZYXWVU987654ZZ".into()),
            asset_class: Some("CRDT".into()),
            contract_type: Some("SWAP".into()),
            value_currency: Some("EUR".into()),
            underlying_id: Some("DE000A1B2C34".into()),
            notional: Some(Decimal::from_str("12500000.00").unwrap()),
            mark_to_market_value: Some(Decimal::from_str("125000.50").unwrap()),
            collateral_value: Some(Decimal::from_str("110000.25").unwrap()),
            ..EmirPositionSetRecord::default()
        };
        let j = serde_json::to_string(&r).unwrap();
        let back: EmirPositionSetRecord = serde_json::from_str(&j).unwrap();
        assert_eq!(back.source_file, r.source_file);
        assert_eq!(back.record_id, r.record_id);
        assert_eq!(back.regime, r.regime);
        assert_eq!(back.reference_date, r.reference_date);
        assert_eq!(back.position_set_kind, r.position_set_kind);
        assert_eq!(back.reporting_counterparty, r.reporting_counterparty);
        assert_eq!(back.other_counterparty, r.other_counterparty);
        assert_eq!(back.asset_class, r.asset_class);
        assert_eq!(back.contract_type, r.contract_type);
        assert_eq!(back.value_currency, r.value_currency);
        assert_eq!(back.underlying_id, r.underlying_id);
        assert_eq!(back.notional, r.notional);
        assert_eq!(back.mark_to_market_value, r.mark_to_market_value);
        assert_eq!(back.collateral_value, r.collateral_value);
    }

    // -- v0.20 A1: EmirQueryRecord (auth.029) ------------------

    #[test]
    fn emir_query_record_default_is_emir_with_all_options_none() {
        let r = EmirQueryRecord::default();
        assert_eq!(r.regime, Regime::Emir);
        assert!(r.source_file.is_none());
        assert!(r.record_id.is_none());
        assert!(r.query_id.is_none());
        assert!(r.query_timestamp.is_none());
        assert!(r.requesting_lei.is_none());
        assert!(r.filter_descriptions.is_empty());
        assert!(r.raw_fields.is_empty());
    }

    #[test]
    fn emir_query_record_serde_always_emits_metadata_keys_when_none() {
        let r = EmirQueryRecord::default();
        let j = serde_json::to_string(&r).unwrap();
        for required_key in [
            "source_file",
            "record_id",
            "query_id",
            "query_timestamp",
            "requesting_lei",
        ] {
            assert!(
                j.contains(&format!("\"{required_key}\":null")),
                "key {required_key} should serialise as null on default, json was {j}"
            );
        }
        // Vec / BTreeMap fields skip when empty.
        assert!(!j.contains("\"filter_descriptions\""));
        assert!(!j.contains("\"raw_fields\""));
    }

    #[test]
    fn emir_query_record_round_trip_populated() {
        use chrono::TimeZone;
        let r = EmirQueryRecord {
            source_file: Some("auth029.xml".into()),
            record_id: Some("auth029.xml#Qry-1".into()),
            regime: Regime::Emir,
            query_id: Some("QRY-2026-001".into()),
            query_timestamp: Some(Utc.with_ymd_and_hms(2026, 5, 21, 8, 0, 0).unwrap()),
            requesting_lei: Some("549300ABCDEFGH123456".into()),
            filter_descriptions: vec![
                "date_range=2026-05-01..2026-05-21".into(),
                "asset_class=CRDT".into(),
            ],
            ..EmirQueryRecord::default()
        };
        let j = serde_json::to_string(&r).unwrap();
        let back: EmirQueryRecord = serde_json::from_str(&j).unwrap();
        assert_eq!(back.source_file, r.source_file);
        assert_eq!(back.record_id, r.record_id);
        assert_eq!(back.regime, r.regime);
        assert_eq!(back.query_id, r.query_id);
        assert_eq!(back.query_timestamp, r.query_timestamp);
        assert_eq!(back.requesting_lei, r.requesting_lei);
        assert_eq!(back.filter_descriptions, r.filter_descriptions);
    }
}
