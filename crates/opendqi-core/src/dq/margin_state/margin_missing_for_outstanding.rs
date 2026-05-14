//! EMIR.MSR.MARGIN_MISSING_FOR_OUTSTANDING — UTI present in the MSR
//! but every margin field is absent.

use super::MarginStateCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, EmirRecord, MarginStateRecord, Regime, Severity};

/// Check implementation.
pub struct EmirMsrMarginMissingForOutstanding;

const CHECK_ID: &str = "EMIR.MSR.MARGIN_MISSING_FOR_OUTSTANDING";

impl MarginStateCheck for EmirMsrMarginMissingForOutstanding {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(
        &self,
        records: &[MarginStateRecord],
        _prior: &[EmirRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        let mut out = Vec::new();
        for r in records {
            let has_any = r.initial_margin_posted_current.is_some()
                || r.initial_margin_collected_current.is_some()
                || r.variation_margin_posted_current.is_some()
                || r.variation_margin_collected_current.is_some();
            if r.uti.is_some() && !has_any {
                out.push(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Emir,
                    severity: Severity::High,
                    dimension: DqDimension::Completeness,
                    record_id: r.record_id.clone(),
                    uti: r.uti.clone(),
                    field: Some("margin".into()),
                    value: None,
                    message: "MSR record carries no margin amount for an outstanding UTI.".into(),
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
    fn flags_when_all_missing() {
        let r = MarginStateRecord {
            uti: Some("U1".into()),
            ..Default::default()
        };
        let out = EmirMsrMarginMissingForOutstanding.run(&[r], &[], &ctx());
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn accepts_some_margin() {
        let r = MarginStateRecord {
            uti: Some("U1".into()),
            initial_margin_posted_current: Some(Decimal::from(1)),
            ..Default::default()
        };
        let out = EmirMsrMarginMissingForOutstanding.run(&[r], &[], &ctx());
        assert!(out.is_empty());
    }
}
