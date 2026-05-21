//! Per-DQI amber / red thresholds and the rate → status mapping.
//!
//! Defaults are tuned conservatively (an EMIR reporting firm
//! with a healthy book should sit `green` on all 10 indicators).
//! Override per-indicator via the `dqi:` block of the YAML
//! config (see [`crate::Thresholds`]).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::model::DqiStatus;

/// Amber / red threshold pair for one DQI.
///
/// Invariant assumed (not enforced — config-author's
/// responsibility): `0.0 ≤ amber ≤ red ≤ 1.0`. If `red < amber`,
/// the mapping degenerates to `red` above `amber` (still valid,
/// just unusual).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DqiThresholdPair {
    /// Boundary above which the status flips from `green` to `amber`.
    pub amber: f64,
    /// Boundary above which the status flips from `amber` to `red`.
    pub red: f64,
}

impl Default for DqiThresholdPair {
    /// Defaults are deliberately strict (5 % amber / 20 % red)
    /// so that a missing per-DQI override always *flags*
    /// rather than silently passes.
    fn default() -> Self {
        Self {
            amber: 0.05,
            red: 0.20,
        }
    }
}

/// Map a `rate` against an amber / red pair to a [`DqiStatus`].
///
/// Boundary semantics (intentional, documented in the plan):
/// - `rate ≤ amber` → [`DqiStatus::Green`]
/// - `amber < rate ≤ red` → [`DqiStatus::Amber`]
/// - `rate > red` → [`DqiStatus::Red`]
/// - `rate.is_none()` → [`DqiStatus::NotApplicable`]
///
/// `NaN` is treated as `NotApplicable` (defensive: a
/// pathological numerator over an empty denominator should
/// not be classified as a breach).
pub fn compute_status(rate: Option<f64>, thresholds: &DqiThresholdPair) -> DqiStatus {
    match rate {
        None => DqiStatus::NotApplicable,
        Some(r) if r.is_nan() => DqiStatus::NotApplicable,
        Some(r) if r <= thresholds.amber => DqiStatus::Green,
        Some(r) if r <= thresholds.red => DqiStatus::Amber,
        Some(_) => DqiStatus::Red,
    }
}

/// Built-in defaults for the v0.15 EMIR Data Quality Pack — one
/// entry per shipped indicator.
///
/// Values are calibrated against ESMA DQ dashboards rule-of-thumb
/// (~5 % amber / ~20 % red for timeliness-style metrics; tighter
/// for completeness-style metrics). Override per-indicator via
/// the YAML config's `dqi:` block; missing entries fall back to
/// [`DqiThresholdPair::default`].
pub fn default_dqi_thresholds() -> BTreeMap<String, DqiThresholdPair> {
    let mut m = BTreeMap::new();
    // Tight: outstanding trades MUST have a valuation.
    m.insert(
        "DQI_VAL_MISSING".into(),
        DqiThresholdPair {
            amber: 0.005,
            red: 0.02,
        },
    );
    m.insert(
        "DQI_VAL_STALE".into(),
        DqiThresholdPair {
            amber: 0.01,
            red: 0.05,
        },
    );
    m.insert(
        "DQI_COL_MISSING_STATE".into(),
        DqiThresholdPair {
            amber: 0.01,
            red: 0.05,
        },
    );
    m.insert(
        "DQI_COL_ALL_ZERO".into(),
        DqiThresholdPair {
            amber: 0.02,
            red: 0.10,
        },
    );
    m.insert(
        "DQI_COL_STALE_STATE".into(),
        DqiThresholdPair {
            amber: 0.05,
            red: 0.20,
        },
    );
    // Rejections matter more than absolute count.
    m.insert(
        "DQI_REJ_RATE".into(),
        DqiThresholdPair {
            amber: 0.01,
            red: 0.05,
        },
    );
    m.insert(
        "DQI_REJ_REPEAT_UTI".into(),
        DqiThresholdPair {
            amber: 0.005,
            red: 0.02,
        },
    );
    m.insert(
        "DQI_TIM_REPORTING_LATE".into(),
        DqiThresholdPair {
            amber: 0.05,
            red: 0.20,
        },
    );
    // Gated metrics — loose defaults so users without the field
    // mapped don't see noise. Status is NotApplicable in that case
    // anyway.
    m.insert(
        "DQI_CONF_MISSING".into(),
        DqiThresholdPair {
            amber: 0.05,
            red: 0.20,
        },
    );
    m.insert(
        "DQI_REC_STATUS_UNPAIRED".into(),
        DqiThresholdPair {
            amber: 0.05,
            red: 0.20,
        },
    );
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(amber: f64, red: f64) -> DqiThresholdPair {
        DqiThresholdPair { amber, red }
    }

    #[test]
    fn rate_none_is_not_applicable() {
        assert_eq!(
            compute_status(None, &pair(0.01, 0.05)),
            DqiStatus::NotApplicable
        );
    }

    #[test]
    fn rate_nan_is_not_applicable() {
        assert_eq!(
            compute_status(Some(f64::NAN), &pair(0.01, 0.05)),
            DqiStatus::NotApplicable
        );
    }

    #[test]
    fn rate_zero_is_green() {
        assert_eq!(
            compute_status(Some(0.0), &pair(0.01, 0.05)),
            DqiStatus::Green
        );
    }

    #[test]
    fn rate_at_amber_boundary_is_green() {
        // ≤ amber → green (boundary inclusive on the green side).
        assert_eq!(
            compute_status(Some(0.01), &pair(0.01, 0.05)),
            DqiStatus::Green
        );
    }

    #[test]
    fn rate_just_above_amber_is_amber() {
        assert_eq!(
            compute_status(Some(0.01000001), &pair(0.01, 0.05)),
            DqiStatus::Amber
        );
    }

    #[test]
    fn rate_at_red_boundary_is_amber() {
        // ≤ red → amber (boundary inclusive on the amber side).
        assert_eq!(
            compute_status(Some(0.05), &pair(0.01, 0.05)),
            DqiStatus::Amber
        );
    }

    #[test]
    fn rate_just_above_red_is_red() {
        assert_eq!(
            compute_status(Some(0.0500001), &pair(0.01, 0.05)),
            DqiStatus::Red
        );
    }

    #[test]
    fn rate_full_is_red() {
        assert_eq!(compute_status(Some(1.0), &pair(0.01, 0.05)), DqiStatus::Red);
    }

    #[test]
    fn default_pair_used_when_indicator_missing() {
        // Sanity: the fallback pair we hand to the orchestrator
        // when the YAML omits a specific indicator.
        let p = DqiThresholdPair::default();
        assert_eq!(p.amber, 0.05);
        assert_eq!(p.red, 0.20);
    }

    #[test]
    fn defaults_cover_all_v015_indicators() {
        let m = default_dqi_thresholds();
        for id in [
            "DQI_VAL_MISSING",
            "DQI_VAL_STALE",
            "DQI_COL_MISSING_STATE",
            "DQI_COL_ALL_ZERO",
            "DQI_COL_STALE_STATE",
            "DQI_REJ_RATE",
            "DQI_REJ_REPEAT_UTI",
            "DQI_TIM_REPORTING_LATE",
            "DQI_CONF_MISSING",
            "DQI_REC_STATUS_UNPAIRED",
        ] {
            assert!(m.contains_key(id), "missing default threshold for {id}");
        }
        assert_eq!(m.len(), 10, "shipping exactly 10 indicators in v0.15");
    }

    #[test]
    fn defaults_satisfy_amber_le_red_invariant() {
        for (id, p) in default_dqi_thresholds() {
            assert!(
                p.amber <= p.red,
                "{id}: amber ({}) must be ≤ red ({})",
                p.amber,
                p.red
            );
        }
    }
}
