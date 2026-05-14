//! EMIR.MSR.COLLATERAL_MARKET_VALUE_NEGATIVE — `collateral_market_value`
//! is negative.

use rust_decimal::Decimal;

use super::MarginStateCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, EmirRecord, MarginStateRecord, Regime, Severity};

/// Check implementation.
pub struct EmirMsrCollateralMarketValueNegative;

const CHECK_ID: &str = "EMIR.MSR.COLLATERAL_MARKET_VALUE_NEGATIVE";

impl MarginStateCheck for EmirMsrCollateralMarketValueNegative {
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
            if let Some(v) = r.collateral_market_value {
                if v < Decimal::ZERO {
                    out.push(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Emir,
                        severity: Severity::Critical,
                        dimension: DqDimension::Accuracy,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("collateral_market_value".into()),
                        value: Some(v.to_string()),
                        message: format!("collateral_market_value is negative: {v}."),
                        source_file: r.source_file.clone(),
                        evidence: Vec::new(),
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
    fn flags_negative() {
        let r = MarginStateRecord {
            collateral_market_value: Some(Decimal::from(-100)),
            ..Default::default()
        };
        let out = EmirMsrCollateralMarketValueNegative.run(&[r], &[], &ctx());
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn accepts_positive() {
        let r = MarginStateRecord {
            collateral_market_value: Some(Decimal::from(1_000)),
            ..Default::default()
        };
        let out = EmirMsrCollateralMarketValueNegative.run(&[r], &[], &ctx());
        assert!(out.is_empty());
    }
}
