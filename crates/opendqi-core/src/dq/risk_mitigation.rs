//! EMIR Article 11 — risk-mitigation checks for non-cleared OTC
//! derivatives. Each check first filters on `clearing_status` and only
//! runs when the trade is non-cleared. Cleared trades are skipped.
//!
//! Notional thresholds (`NFC_ABOVE_CLEARING_THRESHOLD` per asset
//! class and `INITIAL_MARGIN_THRESHOLD`) live in
//! [`crate::config::EmirRmtThresholds`] and are overridable via YAML.
//! See [`docs/emir-risk-mitigation.md`].

use chrono::Duration;
use rust_decimal::Decimal;

use crate::dq::formats::canonical_asset_class;
use crate::dq::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Returns `true` when the trade is reported as non-cleared.
///
/// Accepts EMIR's `clearing_status` codes (`NCLR`, `NCMP`) as well as
/// human shorthand: `non-cleared`, `uncleared`, `false`, `n`, `no`.
pub fn is_uncleared(clearing_status: Option<&str>) -> bool {
    match clearing_status {
        None => false,
        Some(s) => {
            let upper = s.trim().to_uppercase();
            matches!(
                upper.as_str(),
                "NCLR" | "NCMP" | "NON-CLEARED" | "UNCLEARED" | "FALSE" | "N" | "NO"
            )
        }
    }
}

fn issue(
    check_id: &str,
    severity: Severity,
    dimension: DqDimension,
    r: &EmirRecord,
    field: Option<&str>,
    value: Option<String>,
    message: String,
) -> DqIssue {
    DqIssue {
        check_id: check_id.into(),
        regime: Regime::Emir,
        severity,
        dimension,
        record_id: r.record_id.clone(),
        uti: r.uti.clone(),
        field: field.map(String::from),
        value,
        message,
        source_file: r.source_file.clone(),
        evidence: Vec::new(),
    }
}

fn is_outstanding(r: &EmirRecord) -> bool {
    r.termination_date.is_none()
}

// -------- EMIR.RMT.UNCLEARED_NEEDS_CONFIRMATION --------------------

/// Check implementation.
pub struct EmirRmtUnclearedNeedsConfirmation;

impl Check for EmirRmtUnclearedNeedsConfirmation {
    fn id(&self) -> &'static str {
        "EMIR.RMT.UNCLEARED_NEEDS_CONFIRMATION"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| is_uncleared(r.clearing_status.as_deref()))
            .filter(|r| {
                r.confirmation_method
                    .as_deref()
                    .map(|s| s.trim().is_empty())
                    .unwrap_or(true)
            })
            .map(|r| {
                issue(
                    self.id(),
                    self.severity(),
                    self.dimension(),
                    r,
                    Some("confirmation_method"),
                    None,
                    "Non-cleared OTC derivative is missing confirmation_method (EMIR Article 11(1)(a)).".into(),
                )
            })
            .collect()
    }
}

// -------- EMIR.RMT.LATE_CONFIRMATION -------------------------------

/// Check implementation.
pub struct EmirRmtLateConfirmation;

impl Check for EmirRmtLateConfirmation {
    fn id(&self) -> &'static str {
        "EMIR.RMT.LATE_CONFIRMATION"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Timeliness
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        let mut out = Vec::new();
        for r in records
            .iter()
            .filter(|r| is_uncleared(r.clearing_status.as_deref()))
        {
            let (Some(exec), Some(rep)) = (r.execution_timestamp, r.reporting_timestamp) else {
                continue;
            };
            if rep <= exec {
                continue;
            }
            let is_nfc = r
                .nature
                .as_deref()
                .map(|n| n.trim().to_uppercase().starts_with("NFC"))
                .unwrap_or(false);
            let max = if is_nfc {
                Duration::days(2)
            } else {
                Duration::days(1)
            };
            if rep - exec > max {
                out.push(issue(
                    self.id(),
                    self.severity(),
                    self.dimension(),
                    r,
                    Some("reporting_timestamp"),
                    Some(rep.to_rfc3339()),
                    format!(
                        "Confirmation reported {}h after execution, exceeds Article 11(1)(a) deadline ({}h for {}).",
                        (rep - exec).num_hours(),
                        max.num_hours(),
                        if is_nfc { "NFC" } else { "FC" }
                    ),
                ));
            }
        }
        out
    }
}

// -------- EMIR.RMT.PORTFOLIO_RECONCILIATION_MISSING ---------------

/// Check implementation.
pub struct EmirRmtPortfolioReconciliationMissing;

impl Check for EmirRmtPortfolioReconciliationMissing {
    fn id(&self) -> &'static str {
        "EMIR.RMT.PORTFOLIO_RECONCILIATION_MISSING"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| is_uncleared(r.clearing_status.as_deref()))
            .filter(|r| {
                r.initial_margin_posted.is_some()
                    || r.initial_margin_collected.is_some()
                    || r.variation_margin_posted.is_some()
                    || r.variation_margin_collected.is_some()
            })
            .filter(|r| {
                r.collateral_portfolio_code
                    .as_deref()
                    .map(|s| s.trim().is_empty())
                    .unwrap_or(true)
            })
            .map(|r| {
                issue(
                    self.id(),
                    self.severity(),
                    self.dimension(),
                    r,
                    Some("collateral_portfolio_code"),
                    None,
                    "Non-cleared OTC derivative has margin posted/collected but no collateral_portfolio_code — required for Article 11(1)(b) portfolio reconciliation.".into(),
                )
            })
            .collect()
    }
}

// -------- EMIR.RMT.DAILY_VALUATION_MISSING -------------------------

/// Check implementation.
pub struct EmirRmtDailyValuationMissing;

impl Check for EmirRmtDailyValuationMissing {
    fn id(&self) -> &'static str {
        "EMIR.RMT.DAILY_VALUATION_MISSING"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Timeliness
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[EmirRecord], ctx: &CheckContext) -> Vec<DqIssue> {
        let max = Duration::days(1);
        let mut out = Vec::new();
        for r in records
            .iter()
            .filter(|r| is_uncleared(r.clearing_status.as_deref()))
        {
            if !is_outstanding(r) {
                continue;
            }
            match r.valuation_timestamp {
                None => {
                    out.push(issue(
                        self.id(),
                        self.severity(),
                        self.dimension(),
                        r,
                        Some("valuation_timestamp"),
                        None,
                        "Non-cleared outstanding OTC derivative has no valuation_timestamp (Article 11(2) daily valuation).".into(),
                    ));
                }
                Some(vts) if ctx.now - vts > max => {
                    out.push(issue(
                        self.id(),
                        self.severity(),
                        self.dimension(),
                        r,
                        Some("valuation_timestamp"),
                        Some(vts.to_rfc3339()),
                        format!(
                            "Valuation is {}h old, exceeds Article 11(2) daily threshold.",
                            (ctx.now - vts).num_hours()
                        ),
                    ));
                }
                _ => {}
            }
        }
        out
    }
}

// -------- EMIR.RMT.VARIATION_MARGIN_MISSING -----------------------

/// Check implementation.
pub struct EmirRmtVariationMarginMissing;

impl Check for EmirRmtVariationMarginMissing {
    fn id(&self) -> &'static str {
        "EMIR.RMT.VARIATION_MARGIN_MISSING"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| is_uncleared(r.clearing_status.as_deref()))
            .filter(|r| is_outstanding(r))
            .filter(|r| r.variation_margin_posted.is_none() && r.variation_margin_collected.is_none())
            .map(|r| {
                issue(
                    self.id(),
                    self.severity(),
                    self.dimension(),
                    r,
                    Some("variation_margin"),
                    None,
                    "Non-cleared outstanding OTC derivative has no variation margin posted or collected (Article 11(3)).".into(),
                )
            })
            .collect()
    }
}

// -------- EMIR.RMT.INITIAL_MARGIN_THRESHOLD -----------------------

/// Check implementation.
pub struct EmirRmtInitialMarginThreshold;

impl Check for EmirRmtInitialMarginThreshold {
    fn id(&self) -> &'static str {
        "EMIR.RMT.INITIAL_MARGIN_THRESHOLD"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[EmirRecord], ctx: &CheckContext) -> Vec<DqIssue> {
        let aana_eur = ctx.thresholds.emir_rmt.aana_im_threshold_eur;
        let threshold = Decimal::from(aana_eur);
        records
            .iter()
            .filter(|r| is_uncleared(r.clearing_status.as_deref()))
            .filter(|r| r.notional_amount.map(|n| n > threshold).unwrap_or(false))
            .filter(|r| r.initial_margin_posted.is_none() && r.initial_margin_collected.is_none())
            .map(|r| {
                issue(
                    self.id(),
                    self.severity(),
                    self.dimension(),
                    r,
                    Some("initial_margin"),
                    Some(format!(
                        "notional={}",
                        r.notional_amount.map(|n| n.to_string()).unwrap_or_default()
                    )),
                    format!(
                        "Non-cleared OTC derivative with notional > {aana_eur} EUR has no initial margin posted or collected (Article 11(3) AANA-threshold heuristic)."
                    ),
                )
            })
            .collect()
    }
}

// -------- EMIR.RMT.COLLATERAL_CATEGORY_REQUIRED -------------------

/// Check implementation.
pub struct EmirRmtCollateralCategoryRequired;

impl Check for EmirRmtCollateralCategoryRequired {
    fn id(&self) -> &'static str {
        "EMIR.RMT.COLLATERAL_CATEGORY_REQUIRED"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| is_uncleared(r.clearing_status.as_deref()))
            .filter(|r| {
                r.initial_margin_posted.is_some()
                    || r.initial_margin_collected.is_some()
                    || r.variation_margin_posted.is_some()
                    || r.variation_margin_collected.is_some()
            })
            .filter(|r| {
                r.collateralisation_category
                    .as_deref()
                    .map(|s| s.trim().is_empty())
                    .unwrap_or(true)
            })
            .map(|r| {
                issue(
                    self.id(),
                    self.severity(),
                    self.dimension(),
                    r,
                    Some("collateralisation_category"),
                    None,
                    "Non-cleared OTC derivative carries margin but no collateralisation_category — required for risk-mitigation classification.".into(),
                )
            })
            .collect()
    }
}

// -------- EMIR.RMT.NFC_ABOVE_CLEARING_THRESHOLD -------------------

/// Check implementation.
pub struct EmirRmtNfcAboveClearingThreshold;

impl Check for EmirRmtNfcAboveClearingThreshold {
    fn id(&self) -> &'static str {
        "EMIR.RMT.NFC_ABOVE_CLEARING_THRESHOLD"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[EmirRecord], ctx: &CheckContext) -> Vec<DqIssue> {
        let table = &ctx.thresholds.emir_rmt.nfc_clearing_thresholds_eur;
        let mut out = Vec::new();
        for r in records {
            if !is_uncleared(r.clearing_status.as_deref()) {
                continue;
            }
            let is_nfc = r
                .nature
                .as_deref()
                .map(|n| n.trim().eq_ignore_ascii_case("NFC"))
                .unwrap_or(false);
            if !is_nfc {
                continue;
            }
            let Some(asset_class) = r.asset_class.as_deref() else {
                continue;
            };
            let Some(canonical) = canonical_asset_class(asset_class) else {
                continue;
            };
            let Some(&threshold_eur) = table.get(canonical) else {
                continue;
            };
            let threshold = Decimal::from(threshold_eur);
            let Some(notional) = r.notional_amount else {
                continue;
            };
            if notional > threshold {
                out.push(issue(
                    self.id(),
                    self.severity(),
                    self.dimension(),
                    r,
                    Some("nature"),
                    Some(format!("notional={notional}")),
                    format!(
                        "NFC counterparty trading {canonical} uncleared above the {threshold_eur} EUR clearing threshold — verify EMIR Article 10 clearing obligation."
                    ),
                ));
            }
        }
        out
    }
}

// -------- EMIR.RMT.INTRAGROUP_NEEDS_INDICATOR ---------------------

/// Check implementation.
pub struct EmirRmtIntragroupNeedsIndicator;

impl Check for EmirRmtIntragroupNeedsIndicator {
    fn id(&self) -> &'static str {
        "EMIR.RMT.INTRAGROUP_NEEDS_INDICATOR"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| is_uncleared(r.clearing_status.as_deref()))
            .filter(|r| {
                let err = r.entity_responsible_for_reporting.as_deref().map(str::trim);
                let c1 = r.counterparty_1.as_deref().map(str::trim);
                let c2 = r.counterparty_2.as_deref().map(str::trim);
                matches!(err.zip(c1), Some((a, b)) if !a.is_empty() && a == b)
                    || matches!(err.zip(c2), Some((a, b)) if !a.is_empty() && a == b)
            })
            .filter(|r| r.intragroup_indicator.is_none())
            .map(|r| {
                issue(
                    self.id(),
                    self.severity(),
                    self.dimension(),
                    r,
                    Some("intragroup_indicator"),
                    None,
                    "Reporting entity equals one of the counterparties on a non-cleared OTC derivative but intragroup_indicator is missing.".into(),
                )
            })
            .collect()
    }
}

// -------- EMIR.RMT.MASTER_AGREEMENT_REQUIRED ----------------------

/// Check implementation.
pub struct EmirRmtMasterAgreementRequired;

impl Check for EmirRmtMasterAgreementRequired {
    fn id(&self) -> &'static str {
        "EMIR.RMT.MASTER_AGREEMENT_REQUIRED"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| is_uncleared(r.clearing_status.as_deref()))
            .filter(|r| {
                r.master_agreement_type
                    .as_deref()
                    .map(|s| s.trim().is_empty())
                    .unwrap_or(true)
            })
            .map(|r| {
                issue(
                    self.id(),
                    self.severity(),
                    self.dimension(),
                    r,
                    Some("master_agreement_type"),
                    None,
                    "Non-cleared OTC derivative has no master_agreement_type — Article 11 risk mitigation expects a documented agreement.".into(),
                )
            })
            .collect()
    }
}

/// Compression / novation event records (`event_type` ∈ {`COMP`, `NOVA`})
/// must carry both the lineage (`prior_uti`) and the portfolio
/// identifier (`collateral_portfolio_code`) — without them the event
/// is unanalysable for Article 11(1)(c) portfolio-compression
/// follow-up. One issue per missing field is emitted (so a record
/// missing both produces two issues with the same `check_id`).
pub struct EmirRmtCompressionEventIncomplete;

impl Check for EmirRmtCompressionEventIncomplete {
    fn id(&self) -> &'static str {
        "EMIR.RMT.COMPRESSION_EVENT_INCOMPLETE"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        let mut out = Vec::new();
        for r in records
            .iter()
            .filter(|r| is_uncleared(r.clearing_status.as_deref()))
        {
            // Only fires on compression / novation events; other
            // event_types are silently skipped (they carry their own
            // unrelated obligations).
            let is_compression = r
                .event_type
                .as_deref()
                .map(|e| {
                    let e = e.trim();
                    e.eq_ignore_ascii_case("COMP") || e.eq_ignore_ascii_case("NOVA")
                })
                .unwrap_or(false);
            if !is_compression {
                continue;
            }
            let prior_uti_missing = r
                .prior_uti
                .as_deref()
                .map(|s| s.trim().is_empty())
                .unwrap_or(true);
            let portfolio_missing = r
                .collateral_portfolio_code
                .as_deref()
                .map(|s| s.trim().is_empty())
                .unwrap_or(true);
            if prior_uti_missing {
                out.push(issue(
                    self.id(),
                    self.severity(),
                    self.dimension(),
                    r,
                    Some("prior_uti"),
                    None,
                    "Compression / novation event (event_type COMP or NOVA) has no prior_uti — \
                     the lineage required to analyse Article 11(1)(c) portfolio-compression activity is missing."
                        .into(),
                ));
            }
            if portfolio_missing {
                out.push(issue(
                    self.id(),
                    self.severity(),
                    self.dimension(),
                    r,
                    Some("collateral_portfolio_code"),
                    None,
                    "Compression / novation event (event_type COMP or NOVA) has no collateral_portfolio_code — \
                     compression operates at portfolio level (Article 11(1)(c))."
                        .into(),
                ));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, NaiveDate, Utc};

    fn ctx_at(now_iso: &str) -> CheckContext {
        let now = DateTime::parse_from_rfc3339(now_iso)
            .unwrap()
            .with_timezone(&Utc);
        CheckContext {
            thresholds: Default::default(),
            today: now.date_naive(),
            now,
        }
    }

    fn ctx() -> CheckContext {
        ctx_at("2026-05-13T08:00:00Z")
    }

    fn uncleared() -> EmirRecord {
        EmirRecord {
            clearing_status: Some("NCLR".into()),
            ..Default::default()
        }
    }

    fn ts(s: &str) -> Option<DateTime<Utc>> {
        Some(DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc))
    }

    // --- helpers ---

    #[test]
    fn uncleared_codes() {
        for s in ["NCLR", "non-cleared", "false", "NCMP", "no", "uncleared"] {
            assert!(is_uncleared(Some(s)), "{s}");
        }
        for s in ["CLRD", "cleared", "true"] {
            assert!(!is_uncleared(Some(s)), "{s}");
        }
        assert!(!is_uncleared(None));
    }

    // --- 10 paired tests (flag / accept) ---

    #[test]
    fn confirmation_missing_flags_and_accepts() {
        let mut r = uncleared();
        assert_eq!(
            EmirRmtUnclearedNeedsConfirmation
                .run(&[r.clone()], &ctx())
                .len(),
            1
        );
        r.confirmation_method = Some("electronic".into());
        assert!(EmirRmtUnclearedNeedsConfirmation
            .run(&[r], &ctx())
            .is_empty());
    }

    #[test]
    fn late_confirmation_flags_and_accepts() {
        let mut r = uncleared();
        r.nature = Some("FC".into());
        r.execution_timestamp = ts("2026-05-10T08:00:00Z");
        r.reporting_timestamp = ts("2026-05-13T08:00:00Z");
        assert_eq!(EmirRmtLateConfirmation.run(&[r.clone()], &ctx()).len(), 1);
        r.execution_timestamp = ts("2026-05-13T06:00:00Z");
        assert!(EmirRmtLateConfirmation.run(&[r], &ctx()).is_empty());
    }

    #[test]
    fn portfolio_reconciliation_missing_flags_and_accepts() {
        let mut r = uncleared();
        r.initial_margin_posted = Some(Decimal::from(100));
        assert_eq!(
            EmirRmtPortfolioReconciliationMissing
                .run(&[r.clone()], &ctx())
                .len(),
            1
        );
        r.collateral_portfolio_code = Some("P".into());
        assert!(EmirRmtPortfolioReconciliationMissing
            .run(&[r], &ctx())
            .is_empty());
    }

    #[test]
    fn daily_valuation_missing_flags_and_accepts() {
        let mut r = uncleared();
        assert_eq!(
            EmirRmtDailyValuationMissing.run(&[r.clone()], &ctx()).len(),
            1
        );
        r.valuation_timestamp = ts("2026-05-13T07:00:00Z");
        assert!(EmirRmtDailyValuationMissing.run(&[r], &ctx()).is_empty());
    }

    #[test]
    fn variation_margin_missing_flags_and_accepts() {
        let mut r = uncleared();
        assert_eq!(
            EmirRmtVariationMarginMissing
                .run(&[r.clone()], &ctx())
                .len(),
            1
        );
        r.variation_margin_posted = Some(Decimal::from(100));
        assert!(EmirRmtVariationMarginMissing.run(&[r], &ctx()).is_empty());
    }

    #[test]
    fn initial_margin_threshold_flags_and_accepts() {
        let mut r = uncleared();
        r.notional_amount = Some(Decimal::from(10_000_000_000i64));
        assert_eq!(
            EmirRmtInitialMarginThreshold
                .run(&[r.clone()], &ctx())
                .len(),
            1
        );
        r.initial_margin_posted = Some(Decimal::from(1));
        assert!(EmirRmtInitialMarginThreshold.run(&[r], &ctx()).is_empty());
    }

    #[test]
    fn collateral_category_required_flags_and_accepts() {
        let mut r = uncleared();
        r.variation_margin_posted = Some(Decimal::from(100));
        assert_eq!(
            EmirRmtCollateralCategoryRequired
                .run(&[r.clone()], &ctx())
                .len(),
            1
        );
        r.collateralisation_category = Some("FCOL".into());
        assert!(EmirRmtCollateralCategoryRequired
            .run(&[r], &ctx())
            .is_empty());
    }

    #[test]
    fn nfc_above_threshold_flags_and_accepts() {
        let mut r = uncleared();
        r.nature = Some("NFC".into());
        r.asset_class = Some("IR".into());
        // 5 G€ is above the IR default (3 G€) — fires.
        r.notional_amount = Some(Decimal::from(5_000_000_000i64));
        assert_eq!(
            EmirRmtNfcAboveClearingThreshold
                .run(&[r.clone()], &ctx())
                .len(),
            1
        );
        r.notional_amount = Some(Decimal::from(500_000_000i64));
        assert!(EmirRmtNfcAboveClearingThreshold
            .run(&[r], &ctx())
            .is_empty());
    }

    #[test]
    fn nfc_above_threshold_ir_uses_3g_default() {
        // Regression of the v1 false positive: IR at 2 G€ was flagged
        // under the uniform 1 G€ threshold; under the per-class table
        // IR sits at 3 G€, so 2 G€ stays silent.
        let mut r = uncleared();
        r.nature = Some("NFC".into());
        r.asset_class = Some("IR".into());
        r.notional_amount = Some(Decimal::from(2_000_000_000i64));
        assert!(EmirRmtNfcAboveClearingThreshold
            .run(&[r], &ctx())
            .is_empty());
    }

    #[test]
    fn nfc_above_threshold_co_now_fires() {
        // Coverage expansion: commodity wasn't checked in v1 at all.
        // With the per-class table (CO default = 4 G€), a 5 G€ CO trade
        // fires.
        let mut r = uncleared();
        r.nature = Some("NFC".into());
        r.asset_class = Some("CO".into());
        r.notional_amount = Some(Decimal::from(5_000_000_000i64));
        assert_eq!(EmirRmtNfcAboveClearingThreshold.run(&[r], &ctx()).len(), 1);
    }

    #[test]
    fn nfc_above_threshold_respects_yaml_override() {
        // ctx with the IR threshold bumped to 5 G€: an IR trade at
        // 4 G€ that would fire under the default 3 G€ no longer fires.
        let yaml = "emir_rmt:\n  nfc_clearing_thresholds_eur:\n    IR: 5000000000\n";
        let thresholds: crate::config::Thresholds = serde_yaml::from_str(yaml).unwrap();
        let ctx = CheckContext {
            thresholds,
            today: chrono::NaiveDate::from_ymd_opt(2026, 5, 14).unwrap(),
            now: chrono::DateTime::parse_from_rfc3339("2026-05-14T08:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
        };
        let mut r = uncleared();
        r.nature = Some("NFC".into());
        r.asset_class = Some("IR".into());
        r.notional_amount = Some(Decimal::from(4_000_000_000i64));
        assert!(EmirRmtNfcAboveClearingThreshold.run(&[r], &ctx).is_empty());
    }

    #[test]
    fn im_threshold_respects_yaml_override() {
        // ctx with AANA threshold raised to phase-5's 50 G€: a 10 G€
        // trade without IM stops firing.
        let yaml = "emir_rmt:\n  aana_im_threshold_eur: 50000000000\n";
        let thresholds: crate::config::Thresholds = serde_yaml::from_str(yaml).unwrap();
        let ctx = CheckContext {
            thresholds,
            today: chrono::NaiveDate::from_ymd_opt(2026, 5, 14).unwrap(),
            now: chrono::DateTime::parse_from_rfc3339("2026-05-14T08:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
        };
        let mut r = uncleared();
        r.notional_amount = Some(Decimal::from(10_000_000_000i64));
        assert!(EmirRmtInitialMarginThreshold.run(&[r], &ctx).is_empty());
    }

    #[test]
    fn intragroup_needs_indicator_flags_and_accepts() {
        let mut r = uncleared();
        r.entity_responsible_for_reporting = Some("LEI-X".into());
        r.counterparty_1 = Some("LEI-X".into());
        assert_eq!(
            EmirRmtIntragroupNeedsIndicator
                .run(&[r.clone()], &ctx())
                .len(),
            1
        );
        r.intragroup_indicator = Some(true);
        assert!(EmirRmtIntragroupNeedsIndicator.run(&[r], &ctx()).is_empty());
    }

    #[test]
    fn master_agreement_required_flags_and_accepts() {
        let mut r = uncleared();
        assert_eq!(
            EmirRmtMasterAgreementRequired
                .run(&[r.clone()], &ctx())
                .len(),
            1
        );
        r.master_agreement_type = Some("ISDA".into());
        assert!(EmirRmtMasterAgreementRequired.run(&[r], &ctx()).is_empty());
    }

    #[test]
    fn cleared_trades_are_skipped() {
        let mut r = EmirRecord {
            clearing_status: Some("CLRD".into()),
            ..Default::default()
        };
        r.master_agreement_type = None;
        // None of the 10 checks should fire on a cleared trade.
        assert!(EmirRmtUnclearedNeedsConfirmation
            .run(&[r.clone()], &ctx())
            .is_empty());
        assert!(EmirRmtMasterAgreementRequired
            .run(&[r.clone()], &ctx())
            .is_empty());
        assert!(EmirRmtCollateralCategoryRequired
            .run(&[r], &ctx())
            .is_empty());
    }

    // unused imports silenced
    #[allow(dead_code)]
    fn _unused_date_import() -> Option<NaiveDate> {
        None
    }

    // -------- EMIR.RMT.COMPRESSION_EVENT_INCOMPLETE --------

    fn compression_event(uti: &str, event: &str) -> EmirRecord {
        let mut r = uncleared();
        r.uti = Some(uti.into());
        r.event_type = Some(event.into());
        r
    }

    #[test]
    fn compression_missing_both_emits_two_issues() {
        let r = compression_event("U1", "COMP"); // no prior_uti, no portfolio
        let issues = EmirRmtCompressionEventIncomplete.run(&[r], &ctx());
        assert_eq!(issues.len(), 2, "one issue per missing field");
        let fields: Vec<&str> = issues
            .iter()
            .filter_map(|i| i.field.as_deref())
            .collect();
        assert!(fields.contains(&"prior_uti"));
        assert!(fields.contains(&"collateral_portfolio_code"));
    }

    #[test]
    fn nova_with_portfolio_only_flags_prior_uti() {
        let mut r = compression_event("U2", "NOVA");
        r.collateral_portfolio_code = Some("PORT-X".into());
        let issues = EmirRmtCompressionEventIncomplete.run(&[r], &ctx());
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].field.as_deref(), Some("prior_uti"));
    }

    #[test]
    fn compression_with_both_clean() {
        let mut r = compression_event("U3", "COMP");
        r.prior_uti = Some("PRIOR-U3".into());
        r.collateral_portfolio_code = Some("PORT-Y".into());
        assert!(EmirRmtCompressionEventIncomplete
            .run(&[r], &ctx())
            .is_empty());
    }

    #[test]
    fn non_compression_event_skipped() {
        let mut r = uncleared();
        r.uti = Some("U4".into());
        r.event_type = Some("TRAD".into());
        // Missing prior_uti + portfolio — but event_type is not COMP/NOVA.
        assert!(EmirRmtCompressionEventIncomplete
            .run(&[r], &ctx())
            .is_empty());
    }

    #[test]
    fn cleared_compression_skipped() {
        let mut r = compression_event("U5", "COMP");
        r.clearing_status = Some("CLRD".into()); // cleared → skip
        assert!(EmirRmtCompressionEventIncomplete
            .run(&[r], &ctx())
            .is_empty());
    }
}
