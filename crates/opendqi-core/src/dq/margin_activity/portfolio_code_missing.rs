//! EMIR.MAR.PORTFOLIO_CODE_MISSING — `collateral_portfolio_code` is
//! absent on a margin activity record.

use super::MarginActivityCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, MarginActivityRecord, Regime, Severity};

/// Check implementation.
pub struct EmirMarPortfolioCodeMissing;

const CHECK_ID: &str = "EMIR.MAR.PORTFOLIO_CODE_MISSING";

impl MarginActivityCheck for EmirMarPortfolioCodeMissing {
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
        records: &[MarginActivityRecord],
        _prior: &[MarginActivityRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        let mut out = Vec::new();
        for r in records {
            let missing = r
                .collateral_portfolio_code
                .as_deref()
                .map(|s| s.trim().is_empty())
                .unwrap_or(true);
            if missing {
                out.push(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Emir,
                    severity: Severity::High,
                    dimension: DqDimension::Completeness,
                    record_id: r.record_id.clone(),
                    uti: r.uti.clone(),
                    field: Some("collateral_portfolio_code".into()),
                    value: None,
                    message: "Margin activity record has no collateral_portfolio_code.".into(),
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
    fn flags_missing() {
        let r = MarginActivityRecord::default();
        let out = EmirMarPortfolioCodeMissing.run(&[r], &[], &ctx());
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn accepts_present() {
        let r = MarginActivityRecord {
            collateral_portfolio_code: Some("P1".into()),
            ..Default::default()
        };
        let out = EmirMarPortfolioCodeMissing.run(&[r], &[], &ctx());
        assert!(out.is_empty());
    }
}
