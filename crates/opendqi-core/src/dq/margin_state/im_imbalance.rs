//! EMIR.MSR.IM_POSTED_VS_COLLECTED_IMBALANCE — within the same MSR row,
//! IM posted vs IM collected differ by more than 10%.

use super::MarginStateCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, EmirRecord, MarginStateRecord, Regime, Severity};

/// Check implementation.
pub struct EmirMsrImImbalance;

const CHECK_ID: &str = "EMIR.MSR.IM_POSTED_VS_COLLECTED_IMBALANCE";
const RATIO_THRESHOLD: f64 = 0.10;

impl MarginStateCheck for EmirMsrImImbalance {
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
            if let (Some(p), Some(c)) = (
                r.initial_margin_posted_current,
                r.initial_margin_collected_current,
            ) {
                let pf = p.to_string().parse::<f64>().unwrap_or(f64::NAN);
                let cf = c.to_string().parse::<f64>().unwrap_or(f64::NAN);
                if !pf.is_finite() || !cf.is_finite() {
                    continue;
                }
                let denom = pf.abs().max(cf.abs());
                if denom <= f64::EPSILON {
                    continue;
                }
                let diff = (pf - cf).abs() / denom;
                if diff > RATIO_THRESHOLD {
                    out.push(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Emir,
                        severity: Severity::Warning,
                        dimension: DqDimension::Accuracy,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("initial_margin".into()),
                        value: Some(format!("posted={p} collected={c}")),
                        message: format!(
                            "IM posted ({p}) and IM collected ({c}) differ by {:.0}%.",
                            diff * 100.0
                        ),
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
    fn flags_imbalance() {
        let r = MarginStateRecord {
            initial_margin_posted_current: Some(Decimal::from(100)),
            initial_margin_collected_current: Some(Decimal::from(150)),
            ..Default::default()
        };
        let out = EmirMsrImImbalance.run(&[r], &[], &ctx());
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn accepts_close() {
        let r = MarginStateRecord {
            initial_margin_posted_current: Some(Decimal::from(100)),
            initial_margin_collected_current: Some(Decimal::from(105)),
            ..Default::default()
        };
        let out = EmirMsrImImbalance.run(&[r], &[], &ctx());
        assert!(out.is_empty());
    }
}
