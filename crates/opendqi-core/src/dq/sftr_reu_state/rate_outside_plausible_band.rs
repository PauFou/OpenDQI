//! `SFTR.REU.STATE.RATE_OUTSIDE_PLAUSIBLE_BAND` — the cash
//! reinvestment rate on the state snapshot falls outside the
//! conservative band `[-0.05, 0.50]`.
//!
//! State-side mirror of `SFTR.REU.RATE_OUTSIDE_PLAUSIBLE_BAND`
//! (auth.071). Same band, same rationale (unit / sign-error
//! signal), same Warning severity.

use rust_decimal::Decimal;

use super::SftrReuStateCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrReuseStateRecord};

/// Check implementation.
pub struct SftrReuStateRateOutsidePlausibleBand;

const CHECK_ID: &str = "SFTR.REU.STATE.RATE_OUTSIDE_PLAUSIBLE_BAND";

/// Lower inclusive bound. Rates strictly below -5 % are flagged.
const LOWER_BOUND_PCT: i64 = -5; // -0.05

/// Upper inclusive bound. Rates strictly above 50 % are flagged.
const UPPER_BOUND_PCT: i64 = 50; // 0.50

fn lower_bound() -> Decimal {
    Decimal::new(LOWER_BOUND_PCT, 2)
}

fn upper_bound() -> Decimal {
    Decimal::new(UPPER_BOUND_PCT, 2)
}

impl SftrReuStateCheck for SftrReuStateRateOutsidePlausibleBand {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[SftrReuseStateRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
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
                        "cash_reinvestment_rate {rate} on the state snapshot is outside the \
                         plausible band (-0.05, 0.50] — likely a percentage-vs-decimal unit \
                         error or a sign error"
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

    fn rec(rate: Option<Decimal>) -> SftrReuseStateRecord {
        SftrReuseStateRecord {
            record_id: Some("R".into()),
            cash_reinvestment_rate: rate,
            ..Default::default()
        }
    }

    #[test]
    fn does_not_fire_when_rate_is_none() {
        let out = SftrReuStateRateOutsidePlausibleBand.run(&[rec(None)], &ctx());
        assert!(out.is_empty());
    }

    #[test]
    fn accepts_typical_rates_in_band() {
        for r in [
            Decimal::new(125, 4),
            Decimal::new(0, 0),
            Decimal::new(-5, 2),
            Decimal::new(50, 2),
        ] {
            let out = SftrReuStateRateOutsidePlausibleBand.run(&[rec(Some(r))], &ctx());
            assert!(out.is_empty(), "rate {r} should be in band");
        }
    }

    #[test]
    fn fires_on_percentage_unit_error() {
        let out = SftrReuStateRateOutsidePlausibleBand.run(&[rec(Some(Decimal::from(5)))], &ctx());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].severity, Severity::Warning);
    }

    #[test]
    fn fires_on_strongly_negative_rate() {
        let out =
            SftrReuStateRateOutsidePlausibleBand.run(&[rec(Some(Decimal::new(-10, 1)))], &ctx());
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn fires_on_above_upper_bound() {
        let out =
            SftrReuStateRateOutsidePlausibleBand.run(&[rec(Some(Decimal::new(51, 2)))], &ctx());
        assert_eq!(out.len(), 1);
    }
}
