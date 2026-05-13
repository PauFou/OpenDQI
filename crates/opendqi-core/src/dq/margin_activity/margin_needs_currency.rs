//! EMIR.MAR.MARGIN_NEEDS_CURRENCY — a margin amount is present but
//! `margin_currency` is absent.

use super::MarginActivityCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, MarginActivityRecord, Regime, Severity};

/// Check implementation.
pub struct EmirMarMarginNeedsCurrency;

const CHECK_ID: &str = "EMIR.MAR.MARGIN_NEEDS_CURRENCY";

impl MarginActivityCheck for EmirMarMarginNeedsCurrency {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(
        &self,
        records: &[MarginActivityRecord],
        _prior: &[MarginActivityRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        let mut out = Vec::new();
        for r in records {
            let has_any_amount = r.initial_margin_posted.is_some()
                || r.initial_margin_collected.is_some()
                || r.variation_margin_posted.is_some()
                || r.variation_margin_collected.is_some();
            let currency_present = r
                .margin_currency
                .as_deref()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false);
            if has_any_amount && !currency_present {
                out.push(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Emir,
                    severity: Severity::High,
                    dimension: DqDimension::Consistency,
                    record_id: r.record_id.clone(),
                    uti: r.uti.clone(),
                    field: Some("margin_currency".into()),
                    value: None,
                    message: "Margin amount present but margin_currency is missing.".into(),
                    source_file: r.source_file.clone(),
                });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

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
    fn flags_missing_currency() {
        let r = MarginActivityRecord {
            initial_margin_posted: Some(Decimal::from(1000)),
            ..Default::default()
        };
        let out = EmirMarMarginNeedsCurrency.run(&[r], &[], &ctx());
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn accepts_currency_present() {
        let r = MarginActivityRecord {
            initial_margin_posted: Some(Decimal::from(1000)),
            margin_currency: Some("EUR".into()),
            ..Default::default()
        };
        let out = EmirMarMarginNeedsCurrency.run(&[r], &[], &ctx());
        assert!(out.is_empty());
    }
}
