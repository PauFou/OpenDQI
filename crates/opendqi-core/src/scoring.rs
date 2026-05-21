//! Deterministic quality score.
//!
//! The formula is intentionally simple and tunable; it will be
//! refined as more checks come online.

use crate::dq::dqi::{compute_status, DqiStatus, DqiThresholdPair};
use crate::model::{DqIssue, Severity};

/// Compute the 0–100 quality score for a scan.
///
/// ```text
/// score = 100
///         - 25.0 * (critical_count / records)
///         - 10.0 * (high_count     / records)
///         -  3.0 * (warning_count  / records)
///         -  0.5 * (info_count     / records)
/// clamp [0.0, 100.0]
/// ```
///
/// When `records_processed == 0` the score is `100.0`.
pub fn quality_score(records_processed: u32, issues: &[DqIssue]) -> f32 {
    let mut critical = 0u32;
    let mut high = 0u32;
    let mut warning = 0u32;
    let mut info = 0u32;
    for issue in issues {
        match issue.severity {
            Severity::Critical => critical += 1,
            Severity::High => high += 1,
            Severity::Warning => warning += 1,
            Severity::Info => info += 1,
        }
    }
    quality_score_from_counts(records_processed, critical, high, warning, info)
}

/// Same formula as [`quality_score`] but from pre-tallied
/// per-severity counts — so an online accumulator can reproduce the
/// score **byte-identically** without retaining the issues. The sole
/// arithmetic definition; [`quality_score`] delegates here.
pub fn quality_score_from_counts(
    records_processed: u32,
    critical: u32,
    high: u32,
    warning: u32,
    info: u32,
) -> f32 {
    if records_processed == 0 {
        return 100.0;
    }
    let n = records_processed as f32;
    let penalty = 25.0 * (critical as f32 / n)
        + 10.0 * (high as f32 / n)
        + 3.0 * (warning as f32 / n)
        + 0.5 * (info as f32 / n);
    (100.0 - penalty).clamp(0.0, 100.0)
}

/// Safe `numerator / denominator` plus a [`DqiStatus`] derived
/// from a [`DqiThresholdPair`].
///
/// - `denominator == 0` → `(None, NotApplicable)` — by
///   construction there is no rate to evaluate. Callers that
///   need a "zero-out-of-zero is fine" semantics should report
///   `Green` themselves.
/// - otherwise → `(Some(num/denom), status)` where `status` is
///   computed via [`compute_status`]: `≤ amber → Green`,
///   `≤ red → Amber`, else `Red`.
///
/// The division is in `f64` to keep precision over large
/// populations; the caller decides how many decimals to render.
pub fn rate_with_status(
    numerator: u64,
    denominator: u64,
    thresholds: &DqiThresholdPair,
) -> (Option<f64>, DqiStatus) {
    if denominator == 0 {
        return (None, DqiStatus::NotApplicable);
    }
    let rate = numerator as f64 / denominator as f64;
    let status = compute_status(Some(rate), thresholds);
    (Some(rate), status)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DqDimension, Regime};

    fn issue(sev: Severity) -> DqIssue {
        DqIssue {
            check_id: "X".into(),
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
    fn zero_records_yields_perfect_score() {
        assert!((quality_score(0, &[]) - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn perfect_dataset_is_100() {
        assert!((quality_score(10, &[]) - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn one_critical_per_record_clamps_to_75() {
        let issues = vec![issue(Severity::Critical); 1];
        // 100 - 25 * (1/1) = 75
        assert!((quality_score(1, &issues) - 75.0).abs() < 1e-4);
    }

    #[test]
    fn many_critical_clamps_to_zero() {
        let issues = vec![issue(Severity::Critical); 100];
        assert!(quality_score(1, &issues).abs() < f32::EPSILON);
    }

    #[test]
    fn mixed_severities() {
        let mut issues = vec![issue(Severity::Critical)];
        issues.push(issue(Severity::High));
        issues.push(issue(Severity::Warning));
        issues.push(issue(Severity::Info));
        // records=4, penalty = 25*0.25 + 10*0.25 + 3*0.25 + 0.5*0.25 = 9.625
        let s = quality_score(4, &issues);
        assert!((s - 90.375).abs() < 1e-3, "got {s}");
    }

    fn dpair(amber: f64, red: f64) -> DqiThresholdPair {
        DqiThresholdPair { amber, red }
    }

    #[test]
    fn rate_with_status_zero_denom_is_not_applicable() {
        let (rate, status) = rate_with_status(0, 0, &dpair(0.01, 0.05));
        assert_eq!(rate, None);
        assert_eq!(status, DqiStatus::NotApplicable);
    }

    #[test]
    fn rate_with_status_zero_num_is_green() {
        let (rate, status) = rate_with_status(0, 100, &dpair(0.01, 0.05));
        assert_eq!(rate, Some(0.0));
        assert_eq!(status, DqiStatus::Green);
    }

    #[test]
    fn rate_with_status_under_amber_is_green() {
        // 5/1000 = 0.005 ≤ 0.01 → green
        let (rate, status) = rate_with_status(5, 1000, &dpair(0.01, 0.05));
        assert_eq!(rate, Some(0.005));
        assert_eq!(status, DqiStatus::Green);
    }

    #[test]
    fn rate_with_status_between_amber_and_red_is_amber() {
        // 30/1000 = 0.03 ∈ (0.01, 0.05] → amber
        let (_rate, status) = rate_with_status(30, 1000, &dpair(0.01, 0.05));
        assert_eq!(status, DqiStatus::Amber);
    }

    #[test]
    fn rate_with_status_above_red_is_red() {
        // 60/1000 = 0.06 > 0.05 → red
        let (_rate, status) = rate_with_status(60, 1000, &dpair(0.01, 0.05));
        assert_eq!(status, DqiStatus::Red);
    }
}
