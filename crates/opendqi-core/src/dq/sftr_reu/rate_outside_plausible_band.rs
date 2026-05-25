//! `SFTR.REU.RATE_OUTSIDE_PLAUSIBLE_BAND` — the cash
//! reinvestment rate falls outside the **conservative band**
//! `(-0.05, 0.50]` (strictly < -0.05 or strictly > 0.50).
//!
//! The auth.071 XSD declares `CshRinvstmtRate` as a generic
//! `PercentageRate` with no explicit bounds, so any rate is
//! "valid" at the XSD level. In practice plausible cash
//! reinvestment rates on collateral lie between 0 and a few
//! percent ; rates outside the band picked here signal either
//! a unit / sign error (likely the firm reported a percentage
//! instead of a decimal fraction, e.g. `5.0` for 5 %) or a
//! data corruption.
//!
//! Severity = Warning (not Critical) — the value still parses
//! and downstream rate-sensitive DQIs can carry on with their
//! computation. The check is a sanity prompt, not a hard error.

use rust_decimal::Decimal;

use super::SftrReuCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrReuseActivityRecord};

/// Check implementation.
pub struct SftrReuRateOutsidePlausibleBand;

const CHECK_ID: &str = "SFTR.REU.RATE_OUTSIDE_PLAUSIBLE_BAND";

/// Lower exclusive bound. Rates strictly below -5 % are flagged.
const LOWER_BOUND_PCT: i64 = -5; // -0.05

/// Upper inclusive bound. Rates strictly above 50 % are flagged.
const UPPER_BOUND_PCT: i64 = 50; // 0.50

fn lower_bound() -> Decimal {
    // -0.05
    Decimal::new(LOWER_BOUND_PCT, 2)
}

fn upper_bound() -> Decimal {
    // 0.50
    Decimal::new(UPPER_BOUND_PCT, 2)
}

impl SftrReuCheck for SftrReuRateOutsidePlausibleBand {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[SftrReuseActivityRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        let lo = lower_bound();
        let hi = upper_bound();
        let mut out = Vec::new();
        for r in records {
            let Some(rate) = r.cash_reinvestment_rate else {
                continue;
            };
            if rate < lo || rate > hi {
                out.push(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Sftr,
                    severity: Severity::Warning,
                    dimension: DqDimension::Accuracy,
                    record_id: r.record_id.clone(),
                    uti: None,
                    field: Some("cash_reinvestment_rate".into()),
                    value: Some(rate.to_string()),
                    message: format!(
                        "cash_reinvestment_rate {rate} is outside the plausible band \
                         (-0.05, 0.50] — likely a percentage-vs-decimal unit error or \
                         a sign error"
                    ),
                    source_file: r.source_file.clone(),
                    evidence: Vec::new(),
                });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> CheckContext {
        CheckContext {
            thresholds: Default::default(),
            today: chrono::NaiveDate::from_ymd_opt(2026, 5, 13).unwrap(),
            now: chrono::DateTime::parse_from_rfc3339("2026-05-13T08:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
        }
    }

    fn rec(rate: Option<Decimal>) -> SftrReuseActivityRecord {
        SftrReuseActivityRecord {
            record_id: Some("R".into()),
            cash_reinvestment_rate: rate,
            ..Default::default()
        }
    }

    #[test]
    fn does_not_fire_when_rate_is_none() {
        let out = SftrReuRateOutsidePlausibleBand.run(&[rec(None)], &ctx());
        assert!(out.is_empty());
    }

    #[test]
    fn accepts_typical_rates_in_band() {
        // Endpoints handled per the inclusive/exclusive rules
        // documented above: -0.05 is the lower inclusive bound,
        // 0.50 is the upper inclusive bound.
        for r in [
            Decimal::new(125, 4), // 0.0125 (1.25 %)
            Decimal::new(0, 0),   // 0
            Decimal::new(-5, 2),  // -0.05 boundary
            Decimal::new(50, 2),  // 0.50 boundary
        ] {
            let out = SftrReuRateOutsidePlausibleBand.run(&[rec(Some(r))], &ctx());
            assert!(out.is_empty(), "rate {r} should be in band");
        }
    }

    #[test]
    fn fires_on_percentage_unit_error() {
        // A common error: firm reports 5.0 (5 %) instead of
        // 0.05 — the value lands well above 0.50.
        let out = SftrReuRateOutsidePlausibleBand.run(&[rec(Some(Decimal::from(5)))], &ctx());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].severity, Severity::Warning);
    }

    #[test]
    fn fires_on_strongly_negative_rate() {
        let out = SftrReuRateOutsidePlausibleBand.run(&[rec(Some(Decimal::new(-10, 1)))], &ctx()); // -1.0
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn fires_on_above_upper_bound() {
        // 0.51 > 0.50 → fires.
        let out = SftrReuRateOutsidePlausibleBand.run(&[rec(Some(Decimal::new(51, 2)))], &ctx());
        assert_eq!(out.len(), 1);
    }
}
