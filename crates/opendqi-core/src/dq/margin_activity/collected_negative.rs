//! EMIR.MAR.COLLECTED_NEGATIVE — IM or VM collected < 0.

use rust_decimal::Decimal;

use super::MarginActivityCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, MarginActivityRecord, Regime, Severity};

/// Check implementation.
pub struct EmirMarCollectedNegative;

const CHECK_ID: &str = "EMIR.MAR.COLLECTED_NEGATIVE";

impl MarginActivityCheck for EmirMarCollectedNegative {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
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
            for (field, val) in [
                ("initial_margin_collected", r.initial_margin_collected),
                ("variation_margin_collected", r.variation_margin_collected),
            ] {
                if let Some(v) = val {
                    if v < Decimal::ZERO {
                        out.push(DqIssue {
                            check_id: CHECK_ID.into(),
                            regime: Regime::Emir,
                            severity: Severity::High,
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
    fn flags_negative_vm_collected() {
        let r = MarginActivityRecord {
            variation_margin_collected: Some(Decimal::from(-50)),
            ..Default::default()
        };
        let out = EmirMarCollectedNegative.run(&[r], &[], &ctx());
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn accepts_positive() {
        let r = MarginActivityRecord {
            initial_margin_collected: Some(Decimal::from(1000)),
            ..Default::default()
        };
        let out = EmirMarCollectedNegative.run(&[r], &[], &ctx());
        assert!(out.is_empty());
    }
}
