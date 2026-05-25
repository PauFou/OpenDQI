//! `SFTR.MAR.EVENT_WITHOUT_PORTFOLIO` — the MAR `collateral_
//! portfolio_code` is absent.
//!
//! Per the auth.070 XSD `CollPrtflId` is **mandatory** in all 4
//! `TradeReport21Choice__1` wrappers (`New`/`Err`/`Crrctn`/
//! `TradUpd`) — every shape carries it. A record reaching the
//! engine with `None` here is a structural defect upstream
//! (parser miss, malformed XML accepted by the wellformedness
//! check, or hand-constructed record). Without the portfolio
//! identifier the event cannot be linked to a CCP-cleared SFT
//! portfolio for downstream margin reconciliation.

use super::SftrMarCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrMarginActivityRecord};

/// Check implementation.
pub struct SftrMarEventWithoutPortfolio;

const CHECK_ID: &str = "SFTR.MAR.EVENT_WITHOUT_PORTFOLIO";

impl SftrMarCheck for SftrMarEventWithoutPortfolio {
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
            if r.collateral_portfolio_code.is_none() {
                out.push(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Sftr,
                    severity: Severity::High,
                    dimension: DqDimension::Completeness,
                    record_id: r.record_id.clone(),
                    uti: None,
                    field: Some("collateral_portfolio_code".into()),
                    value: None,
                    message: "collateral_portfolio_code is missing on an auth.070 record \
                              (XSD requires CollPrtflId on every wrapper); the MAR event \
                              cannot be linked to a CCP-cleared SFT portfolio"
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
    fn fires_when_portfolio_missing() {
        let r = SftrMarginActivityRecord {
            record_id: Some("R-NOPORT".into()),
            collateral_portfolio_code: None,
            ..Default::default()
        };
        let out = SftrMarEventWithoutPortfolio.run(&[r], &ctx());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].severity, Severity::High);
    }

    #[test]
    fn does_not_fire_when_portfolio_present() {
        let r = SftrMarginActivityRecord {
            collateral_portfolio_code: Some("PORTFOLIO-OK".into()),
            ..Default::default()
        };
        let out = SftrMarEventWithoutPortfolio.run(&[r], &ctx());
        assert!(out.is_empty());
    }
}
