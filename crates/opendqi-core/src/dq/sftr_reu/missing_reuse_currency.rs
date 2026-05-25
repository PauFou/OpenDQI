//! `SFTR.REU.MISSING_REUSE_CURRENCY` — the reuse record carries
//! a `total_reuse_value` but no `reuse_currency`.
//!
//! The ISO 20022 auth.071 XSD requires every `Amt` element to
//! carry a `@Ccy` attribute. The parser promotes the first
//! observed `Scty/ReuseVal/.../@Ccy` onto `reuse_currency`.
//! A record reaching the engine with a positive (Some)
//! total_reuse_value but `reuse_currency=None` means the
//! parser saw no `@Ccy` on any of the reuse amounts — a
//! structural defect upstream.
//!
//! Pure-cash records (total_reuse_value=None) don't fire — the
//! cash side's `RinvstdCsh/@Ccy` lives in raw_fields (not
//! promoted) by design, so a missing typed currency in that
//! shape is not necessarily a defect.

use super::SftrReuCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrReuseActivityRecord};

/// Check implementation.
pub struct SftrReuMissingReuseCurrency;

const CHECK_ID: &str = "SFTR.REU.MISSING_REUSE_CURRENCY";

impl SftrReuCheck for SftrReuMissingReuseCurrency {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[SftrReuseActivityRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
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
                              (XSD violation upstream — every Amt must carry @Ccy)"
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
        let r = SftrReuseActivityRecord {
            record_id: Some("R1".into()),
            total_reuse_value: Some(Decimal::from(1000)),
            reuse_currency: None,
            ..Default::default()
        };
        let out = SftrReuMissingReuseCurrency.run(&[r], &ctx());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].severity, Severity::High);
    }

    #[test]
    fn does_not_fire_when_no_total_reuse_value() {
        // Cash-only record: total_reuse_value=None — no fire.
        let r = SftrReuseActivityRecord {
            record_id: Some("R-CASH".into()),
            cash_reinvestment_rate: Some(Decimal::new(125, 4)),
            ..Default::default()
        };
        let out = SftrReuMissingReuseCurrency.run(&[r], &ctx());
        assert!(out.is_empty());
    }

    #[test]
    fn does_not_fire_when_currency_present() {
        let r = SftrReuseActivityRecord {
            record_id: Some("R-OK".into()),
            total_reuse_value: Some(Decimal::from(1000)),
            reuse_currency: Some("EUR".into()),
            ..Default::default()
        };
        let out = SftrReuMissingReuseCurrency.run(&[r], &ctx());
        assert!(out.is_empty());
    }
}
