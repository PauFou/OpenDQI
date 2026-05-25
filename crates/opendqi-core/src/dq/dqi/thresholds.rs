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
    // v0.17 — SFTR T3 margin layer (auth.085 portfolio-level).
    // Completeness-style margins missing are tighter (~5 % amber
    // / 20 % red) since CCP-cleared SFTs must report margin per
    // RTS 2019/356. The excess-collateral-use rate is set higher
    // (operational-flag, not strict failure) — anything past 50 %
    // of portfolios reporting any XcssColl > 0 is unusual.
    m.insert(
        "DQI_T3_MARGIN_POSTED_MISSING_SFTR".into(),
        DqiThresholdPair {
            amber: 0.05,
            red: 0.20,
        },
    );
    m.insert(
        "DQI_T3_MARGIN_RECEIVED_MISSING_SFTR".into(),
        DqiThresholdPair {
            amber: 0.05,
            red: 0.20,
        },
    );
    m.insert(
        "DQI_T3_EXCESS_COLLATERAL_USE_SFTR".into(),
        DqiThresholdPair {
            amber: 0.20,
            red: 0.50,
        },
    );
    m.insert(
        "DQI_T3_MARGIN_STALE_SFTR".into(),
        DqiThresholdPair {
            amber: 0.05,
            red: 0.20,
        },
    );
    // v0.17 C1 — SFTR auth.080 reconciliation layer (4 DQIs).
    // Calibrated against the existing EMIR auth.091 pairing/
    // reconciliation thresholds — same operational tolerance,
    // mirror across regimes. Higher amber/red than completeness
    // metrics since auth.080 sample sizes can be small per
    // CP-pair (per-day, per-batch).
    m.insert(
        "DQI_PAIRING_RATE_SFTR".into(),
        DqiThresholdPair {
            amber: 0.05,
            red: 0.20,
        },
    );
    m.insert(
        "DQI_RECONCILIATION_RATE_SFTR".into(),
        DqiThresholdPair {
            amber: 0.05,
            red: 0.20,
        },
    );
    m.insert(
        "DQI_UNPAIRED_TRADES_RATE_SFTR".into(),
        DqiThresholdPair {
            amber: 0.05,
            red: 0.20,
        },
    );
    m.insert(
        "DQI_FIELD_MISMATCH_RATE_SFTR".into(),
        DqiThresholdPair {
            amber: 0.05,
            red: 0.20,
        },
    );
    // v0.17 D1 — auth.083 MCR open-requests rollup. Tighter
    // amber/red than reconciliation rates : an open MCR
    // request not visible in TSR is a direct operational
    // failure (counterparty asked for collateral, we have no
    // record of it being posted in the TSR snapshot).
    m.insert(
        "DQI_MCR_OPEN_REQUESTS_SFTR".into(),
        DqiThresholdPair {
            amber: 0.05,
            red: 0.20,
        },
    );
    // v0.17 E1 — 3 SFTR-specific TSR DQIs. HAIRCUT_ANOMALY
    // is tight because [0, 1] is a hard regulatory bound per
    // RTS 2019/356 Art. 4 — any out-of-range value is a
    // structural defect. LEI_MISSING is mirror of EMIR
    // DQI_LEI_MISSING tolerance. UNDER_COLLATERALIZATION is
    // tighter still (0.5% / 2%) since it surfaces direct
    // credit-risk anomalies on top of DQ defects.
    m.insert(
        "DQI_HAIRCUT_ANOMALY_SFTR".into(),
        DqiThresholdPair {
            amber: 0.005,
            red: 0.02,
        },
    );
    m.insert(
        "DQI_LEI_MISSING_SFTR".into(),
        DqiThresholdPair {
            amber: 0.01,
            red: 0.05,
        },
    );
    m.insert(
        "DQI_UNDER_COLLATERALIZATION_SFTR".into(),
        DqiThresholdPair {
            amber: 0.005,
            red: 0.02,
        },
    );
    // v0.18 A4 — 3 SFTR MAR indicators (auth.070 event layer).
    // PARTIAL_SIDES mirrors the T3 completeness baseline (5/20 %).
    // EXCESS_COLLATERAL_EVENT_RATE mirrors the state-side
    // T3_EXCESS_COLLATERAL_USE_SFTR pair (operational signal,
    // looser 20/50 %). EVENT_SPIKE mirrors the reconciliation
    // tolerances (5/20 % — more than one CP-pair in twenty being
    // a 2σ spike is unusual).
    m.insert(
        "DQI_MAR_PARTIAL_SIDES_SFTR".into(),
        DqiThresholdPair {
            amber: 0.05,
            red: 0.20,
        },
    );
    m.insert(
        "DQI_MAR_EXCESS_COLLATERAL_EVENT_RATE_SFTR".into(),
        DqiThresholdPair {
            amber: 0.20,
            red: 0.50,
        },
    );
    m.insert(
        "DQI_MAR_EVENT_SPIKE_SFTR".into(),
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
    fn defaults_cover_all_shipped_indicators() {
        // Locks: every shipped DQI (v0.15 + v0.17) has a baked-in
        // threshold default. Listing each id explicitly is the
        // simplest way to keep the doc-and-test in sync as the
        // catalogue grows ; if you add a new computer, add its
        // id here.
        let m = default_dqi_thresholds();
        for id in [
            // v0.15 — 10 EMIR/feedback indicators
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
            // v0.17 B1' — 4 SFTR T3 margin indicators
            "DQI_T3_MARGIN_POSTED_MISSING_SFTR",
            "DQI_T3_MARGIN_RECEIVED_MISSING_SFTR",
            "DQI_T3_EXCESS_COLLATERAL_USE_SFTR",
            "DQI_T3_MARGIN_STALE_SFTR",
            // v0.17 C1 — 4 SFTR auth.080 reconciliation indicators
            "DQI_PAIRING_RATE_SFTR",
            "DQI_RECONCILIATION_RATE_SFTR",
            "DQI_UNPAIRED_TRADES_RATE_SFTR",
            "DQI_FIELD_MISMATCH_RATE_SFTR",
            // v0.17 D1 — 1 SFTR auth.083 MCR rollup indicator
            "DQI_MCR_OPEN_REQUESTS_SFTR",
            // v0.17 E1 — 3 SFTR-specific TSR indicators
            "DQI_HAIRCUT_ANOMALY_SFTR",
            "DQI_LEI_MISSING_SFTR",
            "DQI_UNDER_COLLATERALIZATION_SFTR",
            // v0.18 A4 — 3 SFTR MAR indicators (auth.070)
            "DQI_MAR_PARTIAL_SIDES_SFTR",
            "DQI_MAR_EXCESS_COLLATERAL_EVENT_RATE_SFTR",
            "DQI_MAR_EVENT_SPIKE_SFTR",
        ] {
            assert!(m.contains_key(id), "missing default threshold for {id}");
        }
        assert_eq!(
            m.len(),
            25,
            "v0.18 A4: v0.17 22 + 3 SFTR MAR indicators = 25"
        );
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
