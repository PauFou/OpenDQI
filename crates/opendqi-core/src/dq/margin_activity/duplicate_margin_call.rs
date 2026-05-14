//! EMIR.MAR.DUPLICATE_MARGIN_CALL — same `collateral_portfolio_code`
//! + same `event_timestamp` repeated in a batch.

use std::collections::BTreeMap;

use super::MarginActivityCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, MarginActivityRecord, Regime, Severity};

/// Check implementation.
pub struct EmirMarDuplicateMarginCall;

const CHECK_ID: &str = "EMIR.MAR.DUPLICATE_MARGIN_CALL";

impl MarginActivityCheck for EmirMarDuplicateMarginCall {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Uniqueness
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
        let mut bucket: BTreeMap<(String, String), Vec<&MarginActivityRecord>> = BTreeMap::new();
        for r in records {
            if let (Some(pc), Some(ev)) =
                (r.collateral_portfolio_code.as_deref(), r.event_timestamp)
            {
                bucket
                    .entry((pc.to_owned(), ev.to_rfc3339()))
                    .or_default()
                    .push(r);
            }
        }
        let mut out = Vec::new();
        for ((pc, ev), bag) in bucket {
            if bag.len() < 2 {
                continue;
            }
            for r in bag {
                out.push(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Emir,
                    severity: Severity::High,
                    dimension: DqDimension::Uniqueness,
                    record_id: r.record_id.clone(),
                    uti: r.uti.clone(),
                    field: Some("event_timestamp".into()),
                    value: Some(ev.clone()),
                    message: format!("Portfolio {pc} has multiple margin events at {ev}."),
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

    fn ts(s: &str) -> Option<DateTime<Utc>> {
        Some(
            chrono::DateTime::parse_from_rfc3339(s)
                .unwrap()
                .with_timezone(&Utc),
        )
    }

    #[test]
    fn flags_duplicates() {
        let a = MarginActivityRecord {
            collateral_portfolio_code: Some("P".into()),
            event_timestamp: ts("2026-05-12T08:00:00Z"),
            ..Default::default()
        };
        let b = a.clone();
        let out = EmirMarDuplicateMarginCall.run(&[a, b], &[], &ctx());
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn accepts_distinct() {
        let a = MarginActivityRecord {
            collateral_portfolio_code: Some("P".into()),
            event_timestamp: ts("2026-05-12T08:00:00Z"),
            ..Default::default()
        };
        let b = MarginActivityRecord {
            collateral_portfolio_code: Some("P".into()),
            event_timestamp: ts("2026-05-13T08:00:00Z"),
            ..Default::default()
        };
        let out = EmirMarDuplicateMarginCall.run(&[a, b], &[], &ctx());
        assert!(out.is_empty());
    }
}
