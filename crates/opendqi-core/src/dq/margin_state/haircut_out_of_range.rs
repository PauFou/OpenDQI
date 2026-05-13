//! EMIR.MSR.HAIRCUT_OUT_OF_RANGE — `haircut_applied` is < 0 or > 1.

use rust_decimal::Decimal;

use super::MarginStateCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, EmirRecord, MarginStateRecord, Regime, Severity};

/// Check implementation.
pub struct EmirMsrHaircutOutOfRange;

const CHECK_ID: &str = "EMIR.MSR.HAIRCUT_OUT_OF_RANGE";

impl MarginStateCheck for EmirMsrHaircutOutOfRange {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(
        &self,
        records: &[MarginStateRecord],
        _prior: &[EmirRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        let mut out = Vec::new();
        for r in records {
            if let Some(h) = r.haircut_applied {
                if h < Decimal::ZERO || h > Decimal::ONE {
                    out.push(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Emir,
                        severity: Severity::Warning,
                        dimension: DqDimension::Accuracy,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("haircut_applied".into()),
                        value: Some(h.to_string()),
                        message: format!("Haircut {h} is out of [0, 1]."),
                        source_file: r.source_file.clone(),
                    });
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
    fn flags_above_one() {
        let r = MarginStateRecord {
            haircut_applied: Some(Decimal::new(15, 1)),
            ..Default::default()
        };
        let out = EmirMsrHaircutOutOfRange.run(&[r], &[], &ctx());
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn accepts_in_range() {
        let r = MarginStateRecord {
            haircut_applied: Some(Decimal::new(5, 2)),
            ..Default::default()
        };
        let out = EmirMsrHaircutOutOfRange.run(&[r], &[], &ctx());
        assert!(out.is_empty());
    }
}
