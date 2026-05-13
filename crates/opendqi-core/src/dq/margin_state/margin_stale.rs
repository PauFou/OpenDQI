//! EMIR.MSR.MARGIN_STALE — header `state_as_of` is older than the
//! configured threshold relative to `ctx.now`.

use chrono::Duration;

use super::MarginStateCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, EmirRecord, MarginStateRecord, Regime, Severity};

/// Check implementation.
pub struct EmirMsrMarginStale;

const CHECK_ID: &str = "EMIR.MSR.MARGIN_STALE";

impl MarginStateCheck for EmirMsrMarginStale {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Timeliness
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(
        &self,
        records: &[MarginStateRecord],
        _prior: &[EmirRecord],
        ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        let max_days = ctx.thresholds.timeliness.max_valuation_age_business_days;
        let mut out = Vec::new();
        for r in records {
            if let Some(state) = r.state_as_of {
                let age = ctx.now - state;
                if age > Duration::days(max_days) {
                    out.push(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Emir,
                        severity: Severity::High,
                        dimension: DqDimension::Timeliness,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("state_as_of".into()),
                        value: Some(state.to_rfc3339()),
                        message: format!(
                            "MSR `state_as_of` is {} days old (threshold {max_days}).",
                            age.num_days()
                        ),
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
    use chrono::{DateTime, Utc};

    fn ctx() -> CheckContext {
        CheckContext {
            thresholds: Default::default(),
            today: chrono::NaiveDate::from_ymd_opt(2026, 5, 13).unwrap(),
            now: chrono::DateTime::parse_from_rfc3339("2026-05-13T08:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
        }
    }

    fn ts(s: &str) -> DateTime<Utc> {
        chrono::DateTime::parse_from_rfc3339(s)
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn flags_stale() {
        let r = MarginStateRecord {
            state_as_of: Some(ts("2026-04-01T08:00:00Z")),
            ..Default::default()
        };
        let out = EmirMsrMarginStale.run(&[r], &[], &ctx());
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn accepts_fresh() {
        let r = MarginStateRecord {
            state_as_of: Some(ts("2026-05-13T08:00:00Z")),
            ..Default::default()
        };
        let out = EmirMsrMarginStale.run(&[r], &[], &ctx());
        assert!(out.is_empty());
    }
}
