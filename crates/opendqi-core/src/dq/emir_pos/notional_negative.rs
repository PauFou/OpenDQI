//! `EMIR.POS.NOTIONAL_NEGATIVE` — the position-set record
//! reports a strictly-negative `notional` amount.
//!
//! Per-record version of the aggregate `DQI_POSITION_NOTIONAL_
//! NEGATIVE` (E4). The DQI rolls up the rate; this check
//! surfaces the exact violating row. Critical: a negative
//! aggregate notional has no regulatory meaning at the
//! position-set level — it's a structural defect or sign error
//! that should block the report from being treated as valid
//! input downstream.

use rust_decimal::Decimal;

use super::EmirPosCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, EmirPositionSetRecord, Regime, Severity};

/// Check implementation.
pub struct EmirPosNotionalNegative;

const CHECK_ID: &str = "EMIR.POS.NOTIONAL_NEGATIVE";

impl EmirPosCheck for EmirPosNotionalNegative {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
    }
    fn severity(&self) -> Severity {
        Severity::Critical
    }
    fn run(&self, records: &[EmirPositionSetRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        let mut out = Vec::new();
        for r in records {
            let Some(n) = r.notional else { continue };
            if n < Decimal::ZERO {
                out.push(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Emir,
                    severity: Severity::Critical,
                    dimension: DqDimension::Accuracy,
                    record_id: r.record_id.clone(),
                    uti: None,
                    field: Some("notional".into()),
                    value: Some(n.to_string()),
                    message: format!(
                        "{} record reports negative notional {n}",
                        r.position_set_kind.as_deref().unwrap_or("position-set")
                    ),
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
            today: chrono::NaiveDate::from_ymd_opt(2026, 5, 21).unwrap(),
            now: chrono::DateTime::parse_from_rfc3339("2026-05-21T00:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
        }
    }

    #[test]
    fn fires_on_negative_notional() {
        let r = EmirPositionSetRecord {
            record_id: Some("R-NEG".into()),
            position_set_kind: Some("PosSet".into()),
            notional: Some(Decimal::from(-100)),
            ..Default::default()
        };
        let out = EmirPosNotionalNegative.run(&[r], &ctx());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].severity, Severity::Critical);
    }

    #[test]
    fn does_not_fire_on_zero_or_positive() {
        let recs = vec![
            EmirPositionSetRecord {
                notional: Some(Decimal::ZERO),
                ..Default::default()
            },
            EmirPositionSetRecord {
                notional: Some(Decimal::from(100)),
                ..Default::default()
            },
        ];
        let out = EmirPosNotionalNegative.run(&recs, &ctx());
        assert!(out.is_empty());
    }

    #[test]
    fn does_not_fire_when_notional_is_none() {
        let r = EmirPositionSetRecord {
            notional: None,
            ..Default::default()
        };
        let out = EmirPosNotionalNegative.run(&[r], &ctx());
        assert!(out.is_empty());
    }
}
