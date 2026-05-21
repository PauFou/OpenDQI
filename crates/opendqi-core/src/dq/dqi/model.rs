//! Types for the Data Quality Pack output.
//!
//! All types are `serde`-friendly so that they round-trip through
//! YAML (config), JSON (`summary.json`), and CSV (the new
//! `indicators.csv` + `evidence.csv` artifacts).

use serde::{Deserialize, Serialize};

use crate::model::{DqDimension, Regime};

/// Status of a single [`DqiIndicator`] — the regulator-style
/// green / amber / red bucket derived from `rate` vs the
/// indicator's amber / red thresholds.
///
/// `NotApplicable` is **not** a missing value; it explicitly
/// signals "this indicator cannot be computed on the inputs
/// provided" (e.g. the layer was not passed, or a gated field
/// like `confirmation_timestamp` was absent from the mapping).
/// It is reported with `rate = None` and **no evidence rows**.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DqiStatus {
    /// `rate ≤ amber_threshold` — within target.
    Green,
    /// `amber_threshold < rate ≤ red_threshold` — investigate.
    Amber,
    /// `rate > red_threshold` — breach.
    Red,
    /// Denominator zero, layer absent, or gating field missing.
    NotApplicable,
}

impl std::fmt::Display for DqiStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DqiStatus::Green => f.write_str("green"),
            DqiStatus::Amber => f.write_str("amber"),
            DqiStatus::Red => f.write_str("red"),
            DqiStatus::NotApplicable => f.write_str("not_applicable"),
        }
    }
}

/// One aggregated Data Quality Indicator.
///
/// Schema is **v1.0 stable** (committee + regulator-readable).
/// Breaking changes require a major version bump and the
/// `test_indicators_schema_matches_csv_golden` parity test will
/// catch silent drift.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DqiIndicator {
    /// Stable identifier, e.g. `"DQI_VAL_MISSING"`. Uppercase
    /// snake-case, dot-free.
    pub indicator_id: String,
    /// Regime under inspection.
    pub regime: Regime,
    /// Primary DQ dimension (one of the six pillars).
    pub dimension: DqDimension,
    /// Which TR layer(s) the indicator reads (e.g. `"TSR"`,
    /// `"MSR"`, `"TSR+MSR"`, `"Feedback"`). Free-form short
    /// label — formatted for display, not parsed.
    pub table_scope: String,
    /// Number of violating records.
    pub numerator: u64,
    /// Eligible population (denominator basis).
    pub denominator: u64,
    /// `numerator / denominator` when `denominator > 0`,
    /// otherwise `None`. Float for display; not used as a
    /// reduction key.
    pub rate: Option<f64>,
    /// Amber threshold applied. `None` when the indicator
    /// shipped no default (rare — defensive against missing
    /// thresholds config).
    pub threshold_amber: Option<f64>,
    /// Red threshold applied.
    pub threshold_red: Option<f64>,
    /// Derived green / amber / red / not-applicable bucket.
    pub status: DqiStatus,
    /// Plain-English description (renderable as-is in the HTML
    /// report). Each indicator ships a static description.
    pub description: String,
}

/// One piece of drill-down evidence supporting a [`DqiIndicator`].
///
/// At most `top-20` per indicator in v0.15. Sorted by indicator-
/// specific priority (e.g. oldest first for `*_STALE`, biggest
/// delay first for `*_LATE`).
///
/// Schema is **v1.0 stable** (mirrors `evidence.csv`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DqiEvidence {
    /// Foreign key into the indicators table.
    pub indicator_id: String,
    /// UTI of the violating record. Always present for v0.15
    /// (no UTI-less indicators yet).
    pub uti: String,
    /// Counterparty LEI when available (often the reporting
    /// counterparty, sometimes the other counterparty depending
    /// on layer).
    pub counterparty: Option<String>,
    /// Asset class short code when available.
    pub asset_class: Option<String>,
    /// Source file the record came from (one of the input
    /// XML / Parquet / CSV paths).
    pub source_file: Option<String>,
    /// The offending value, stringified — what makes this row
    /// violate the indicator. e.g. a missing-valuation record
    /// has `observed_value = None`, a stale-valuation has
    /// `observed_value = Some("2025-02-01")` (the old timestamp).
    pub observed_value: Option<String>,
    /// One-line human explanation, e.g.
    /// `"valuation_timestamp older than 1 business day"`.
    pub explanation: String,
}

/// Lightweight signal from the orchestrator to the gated
/// computers ([`DQI_CONF_MISSING`], [`DQI_REC_STATUS_UNPAIRED`])
/// telling them whether the underlying field is actually
/// present in the user's mapping + the Arrow / record schema.
///
/// When `false`, the gated computer returns
/// `status: NotApplicable` with `rate: None` and **no
/// evidence**, plus an `explanation`-ready description
/// indicating which field was missing.
///
/// [`DQI_CONF_MISSING`]: crate::dq::dqi
/// [`DQI_REC_STATUS_UNPAIRED`]: crate::dq::dqi
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MappingPresence {
    /// `true` iff `confirmation_timestamp` is mapped AND the
    /// column exists in the record schema AND ≥ 1 non-NULL
    /// value was observed in the first batch.
    pub has_confirmation_timestamp: bool,
    /// `true` iff `reconciliation_status` is mapped AND
    /// observed at least once non-NULL.
    pub has_reconciliation_status: bool,
}

/// Full result of [`crate::dq::dqi::compute_emir_dqi_pack`]
/// (added in D4).
///
/// Decoupled from [`crate::ScanSummary`]: the issues stream
/// (granular [`crate::DqIssue`]s + their summary) lives in
/// `issues_summary` + `issues`; the **new** aggregated layer
/// lives in `indicators` + `evidence`. Both ship from the
/// same orchestrator call.
///
/// `PartialEq` is intentionally **not** derived: [`crate::DqIssue`]
/// and [`crate::ScanSummary`] do not implement it (issues carry
/// `Decimal` / chrono types compared via the shared 8-field
/// comparator). Tests should compare the constituent fields
/// directly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DqiPackResult {
    /// One row per DQI computed for the inputs provided.
    /// Indicators that could not be computed appear with
    /// `status: NotApplicable` rather than being omitted —
    /// downstream consumers can rely on a fixed indicator set
    /// for a given regime.
    pub indicators: Vec<DqiIndicator>,
    /// Drill-down evidence, ≤ 20 rows per indicator. May be
    /// empty for indicators with no violations or with
    /// `status: NotApplicable`.
    pub evidence: Vec<DqiEvidence>,
    /// Existing-shape scan summary from running the **216
    /// granular checks** in parallel on each input layer.
    /// The DQI pack does not replace this stream — it adds a
    /// committee-readable layer on top.
    pub issues_summary: crate::model::ScanSummary,
    /// Granular issues as a flat vector (already sorted by
    /// the shared 8-field comparator). Carries the same
    /// content as `issues.csv` would.
    pub issues: Vec<crate::model::DqIssue>,
}
