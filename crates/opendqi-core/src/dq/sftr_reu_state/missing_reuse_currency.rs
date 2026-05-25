//! `SFTR.REU.STATE.MISSING_REUSE_CURRENCY` — the reuse-state
//! snapshot carries a `total_reuse_value` but no `reuse_currency`.
//!
//! State-side mirror of `SFTR.REU.MISSING_REUSE_CURRENCY`
//! (auth.071). Same semantics: a record with reported reuse
//! amounts but no currency means the parser saw no `@Ccy` on
//! any of the `Scty/ReuseVal/.../Amt` elements — XSD violation
//! upstream.

use super::SftrReuStateCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrReuseStateRecord};

/// Check implementation.
pub struct SftrReuStateMissingReuseCurrency;

const CHECK_ID: &str = "SFTR.REU.STATE.MISSING_REUSE_CURRENCY";

impl SftrReuStateCheck for SftrReuStateMissingReuseCurrency {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[SftrReuseStateRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        let mut out = Vec::new();
        for r in records {
            if r.total_reuse_value.is_some() && r.reuse_currency.is_none() {
                out.push(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Sftr,
                    severity: Severity::High,
                    dimension: DqDimension::Completeness,
                    record_id: r.record_id.clone(),
                    uti: None,
                    field: Some("reuse_currency".into()),
                    value: None,
                    message: "reuse_currency is missing while total_reuse_value is reported \
                              on the state snapshot (XSD violation upstream — every Amt must \
                              carry @Ccy)"
                        .into(),
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
    use rust_decimal::Decimal;

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

    #[test]
    fn fires_when_total_set_but_currency_missing() {
        let r = SftrReuseStateRecord {
            record_id: Some("R1".into()),
            total_reuse_value: Some(Decimal::from(1000)),
            reuse_currency: None,
            ..Default::default()
        };
        let out = SftrReuStateMissingReuseCurrency.run(&[r], &ctx());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].severity, Severity::High);
    }

    #[test]
    fn does_not_fire_when_no_total_reuse_value() {
        // Cash-only snapshot: total_reuse_value=None — no fire.
        let r = SftrReuseStateRecord {
            record_id: Some("R-CASH".into()),
            cash_reinvestment_rate: Some(Decimal::new(125, 4)),
            ..Default::default()
        };
        let out = SftrReuStateMissingReuseCurrency.run(&[r], &ctx());
        assert!(out.is_empty());
    }

    #[test]
    fn does_not_fire_when_currency_present() {
        let r = SftrReuseStateRecord {
            record_id: Some("R-OK".into()),
            total_reuse_value: Some(Decimal::from(1000)),
            reuse_currency: Some("EUR".into()),
            ..Default::default()
        };
        let out = SftrReuStateMissingReuseCurrency.run(&[r], &ctx());
        assert!(out.is_empty());
    }
}
