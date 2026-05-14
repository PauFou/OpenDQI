//! EMIR.MSR.VARIATION_MARGIN_NEGATIVE — current VM posted or collected
//! is negative.

use rust_decimal::Decimal;

use super::MarginStateCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, EmirRecord, MarginStateRecord, Regime, Severity};

/// Check implementation.
pub struct EmirMsrVariationMarginNegative;

const CHECK_ID: &str = "EMIR.MSR.VARIATION_MARGIN_NEGATIVE";

impl MarginStateCheck for EmirMsrVariationMarginNegative {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
    }
    fn severity(&self) -> Severity {
        Severity::Critical
    }
    fn run(
        &self,
        records: &[MarginStateRecord],
        _prior: &[EmirRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        let mut out = Vec::new();
        for r in records {
            for (field, val) in [
                (
                    "variation_margin_posted_current",
                    r.variation_margin_posted_current,
                ),
                (
                    "variation_margin_collected_current",
                    r.variation_margin_collected_current,
                ),
            ] {
                if let Some(v) = val {
                    if v < Decimal::ZERO {
                        out.push(DqIssue {
                            check_id: CHECK_ID.into(),
                            regime: Regime::Emir,
                            severity: Severity::Critical,
                            dimension: DqDimension::Accuracy,
                            record_id: r.record_id.clone(),
                            uti: r.uti.clone(),
                            field: Some(field.into()),
                            value: Some(v.to_string()),
                            message: format!("{field} is negative: {v}."),
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
    fn flags_negative_vm() {
        let r = MarginStateRecord {
            variation_margin_collected_current: Some(Decimal::from(-1)),
            ..Default::default()
        };
        let out = EmirMsrVariationMarginNegative.run(&[r], &[], &ctx());
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn accepts_zero() {
        let r = MarginStateRecord {
            variation_margin_posted_current: Some(Decimal::ZERO),
            ..Default::default()
        };
        let out = EmirMsrVariationMarginNegative.run(&[r], &[], &ctx());
        assert!(out.is_empty());
    }
}
