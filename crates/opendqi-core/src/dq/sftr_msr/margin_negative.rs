//! `SFTR.T3.MARGIN_NEGATIVE` — any of the 6 SFTR MSR amounts
//! (IM/VM posted/received + excess collateral posted/received)
//! reported with a strictly negative value.
//!
//! Severity = Critical : a negative margin amount is a
//! structural reporting defect that breaks downstream
//! reconciliation arithmetic.

use rust_decimal::Decimal;

use super::SftrMsrCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrMarginStateRecord};

/// Check implementation.
pub struct SftrT3MarginNegative;

const CHECK_ID: &str = "SFTR.T3.MARGIN_NEGATIVE";

impl SftrMsrCheck for SftrT3MarginNegative {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
    }
    fn severity(&self) -> Severity {
        Severity::Critical
    }
    fn run(&self, records: &[SftrMarginStateRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        let mut out = Vec::new();
        for r in records {
            for (field, val) in [
                ("initial_margin_posted", r.initial_margin_posted),
                ("variation_margin_posted", r.variation_margin_posted),
                ("excess_collateral_posted", r.excess_collateral_posted),
                ("initial_margin_received", r.initial_margin_received),
                ("variation_margin_received", r.variation_margin_received),
                ("excess_collateral_received", r.excess_collateral_received),
            ] {
                if let Some(v) = val {
                    if v < Decimal::ZERO {
                        out.push(DqIssue {
                            check_id: CHECK_ID.into(),
                            regime: Regime::Sftr,
                            severity: Severity::Critical,
                            dimension: DqDimension::Accuracy,
                            record_id: r.record_id.clone(),
                            uti: r.collateral_portfolio_code.clone(),
                            field: Some(field.into()),
                            value: Some(v.to_string()),
                            message: format!("{field} is negative: {v}"),
                            source_file: r.source_file.clone(),
                            evidence: Vec::new(),
                        });
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

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
    fn flags_each_negative_field_independently() {
        let r = SftrMarginStateRecord {
            collateral_portfolio_code: Some("P1".into()),
            initial_margin_posted: Some(Decimal::from_str("-50").unwrap()),
            variation_margin_received: Some(Decimal::from_str("-10").unwrap()),
            excess_collateral_posted: Some(Decimal::from(100)), // positive, no flag
            ..Default::default()
        };
        let out = SftrT3MarginNegative.run(&[r], &ctx());
        assert_eq!(out.len(), 2);
        let fields: Vec<&str> = out.iter().map(|i| i.field.as_deref().unwrap()).collect();
        assert!(fields.contains(&"initial_margin_posted"));
        assert!(fields.contains(&"variation_margin_received"));
    }

    #[test]
    fn accepts_positive_amounts() {
        let r = SftrMarginStateRecord {
            collateral_portfolio_code: Some("P1".into()),
            initial_margin_posted: Some(Decimal::from(1000)),
            variation_margin_posted: Some(Decimal::from(50)),
            ..Default::default()
        };
        let out = SftrT3MarginNegative.run(&[r], &ctx());
        assert!(out.is_empty());
    }

    #[test]
    fn accepts_zero_amounts() {
        // Strict negative — zero is a legitimate value (e.g.
        // freshly initialised portfolio with no margin yet).
        let r = SftrMarginStateRecord {
            collateral_portfolio_code: Some("P1".into()),
            initial_margin_posted: Some(Decimal::ZERO),
            ..Default::default()
        };
        let out = SftrT3MarginNegative.run(&[r], &ctx());
        assert!(out.is_empty());
    }
}
