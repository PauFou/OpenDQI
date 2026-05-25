//! `SFTR.MAR.AMOUNT_CURRENCY_MISSING` — at least one MAR amount
//! is set but `margin_currency` is `None`. Activity-side mirror
//! of `SFTR.T3.MARGIN_CURRENCY_MISSING` (auth.085 state).
//!
//! The ISO 20022 XSD requires every `Amt` element to carry a
//! `@Ccy` attribute, so the parser promotes the first observed
//! `@Ccy` onto `margin_currency`. A record reaching the engine
//! with any amount but no currency means the parser saw no
//! `@Ccy` at all — a structural defect upstream.

use super::SftrMarCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrMarginActivityRecord};

/// Check implementation.
pub struct SftrMarAmountCurrencyMissing;

const CHECK_ID: &str = "SFTR.MAR.AMOUNT_CURRENCY_MISSING";

fn has_any_amount(r: &SftrMarginActivityRecord) -> bool {
    r.initial_margin_posted.is_some()
        || r.variation_margin_posted.is_some()
        || r.excess_collateral_posted.is_some()
        || r.initial_margin_received.is_some()
        || r.variation_margin_received.is_some()
        || r.excess_collateral_received.is_some()
}

impl SftrMarCheck for SftrMarAmountCurrencyMissing {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[SftrMarginActivityRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        let mut out = Vec::new();
        for r in records {
            if has_any_amount(r) && r.margin_currency.is_none() {
                out.push(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Sftr,
                    severity: Severity::High,
                    dimension: DqDimension::Completeness,
                    record_id: r.record_id.clone(),
                    uti: r.collateral_portfolio_code.clone(),
                    field: Some("margin_currency".into()),
                    value: None,
                    message: "margin_currency is missing while at least one MAR amount is \
                              reported (XSD violation upstream)"
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
    fn fires_when_amount_set_but_currency_missing() {
        let r = SftrMarginActivityRecord {
            collateral_portfolio_code: Some("P1".into()),
            initial_margin_posted: Some(Decimal::from(100)),
            margin_currency: None,
            ..Default::default()
        };
        let out = SftrMarAmountCurrencyMissing.run(&[r], &ctx());
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn does_not_fire_when_no_amount_present_eg_err_wrapper() {
        // Err wrapper carries no amounts and no @Ccy — neither
        // a violation nor a fire.
        let r = SftrMarginActivityRecord {
            collateral_portfolio_code: Some("P-ERR".into()),
            action_type: Some("ERRT".into()),
            ..Default::default()
        };
        let out = SftrMarAmountCurrencyMissing.run(&[r], &ctx());
        assert!(out.is_empty());
    }

    #[test]
    fn does_not_fire_when_currency_present() {
        let r = SftrMarginActivityRecord {
            collateral_portfolio_code: Some("P1".into()),
            initial_margin_posted: Some(Decimal::from(100)),
            margin_currency: Some("EUR".into()),
            ..Default::default()
        };
        let out = SftrMarAmountCurrencyMissing.run(&[r], &ctx());
        assert!(out.is_empty());
    }
}
