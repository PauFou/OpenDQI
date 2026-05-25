//! `SFTR.MAR.EVENT_DATE_IN_FUTURE` — the MAR `event_date`, when
//! present, lies strictly after the `CheckContext::today`
//! reference date.
//!
//! Per the auth.070 XSD an `Err` wrapper carries no `EvtDt` so
//! the check naturally skips those records (event_date is
//! `None`). On the 3 non-Err wrappers (`New`/`Crrctn`/`TradUpd`)
//! `EvtDt` is structurally present per the XSD, so `None` here
//! means the parser failed to map it — that's surfaced by the
//! ESMA-format XSD-validation layer rather than this check.

use super::SftrMarCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrMarginActivityRecord};

/// Check implementation.
pub struct SftrMarEventDateInFuture;

const CHECK_ID: &str = "SFTR.MAR.EVENT_DATE_IN_FUTURE";

impl SftrMarCheck for SftrMarEventDateInFuture {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Validity
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[SftrMarginActivityRecord], ctx: &CheckContext) -> Vec<DqIssue> {
        let mut out = Vec::new();
        for r in records {
            if let Some(evt) = r.event_date {
                if evt > ctx.today {
                    out.push(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Sftr,
                        severity: Severity::High,
                        dimension: DqDimension::Validity,
                        record_id: r.record_id.clone(),
                        uti: r.collateral_portfolio_code.clone(),
                        field: Some("event_date".into()),
                        value: Some(evt.format("%Y-%m-%d").to_string()),
                        message: format!(
                            "event_date {evt} is later than the reference date {today}",
                            today = ctx.today
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
    use chrono::NaiveDate;

    fn ctx_on(date: NaiveDate) -> CheckContext {
        CheckContext {
            thresholds: Default::default(),
            today: date,
            now: date.and_hms_opt(0, 0, 0).unwrap().and_utc(),
        }
    }

    #[test]
    fn fires_on_future_event_date() {
        let r = SftrMarginActivityRecord {
            record_id: Some("R-FUT".into()),
            collateral_portfolio_code: Some("P1".into()),
            event_date: Some(NaiveDate::from_ymd_opt(2026, 5, 30).unwrap()),
            ..Default::default()
        };
        let out = SftrMarEventDateInFuture
            .run(&[r], &ctx_on(NaiveDate::from_ymd_opt(2026, 5, 13).unwrap()));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].check_id, "SFTR.MAR.EVENT_DATE_IN_FUTURE");
        assert_eq!(out[0].severity, Severity::High);
    }

    #[test]
    fn does_not_fire_on_today_or_past() {
        let today = NaiveDate::from_ymd_opt(2026, 5, 13).unwrap();
        let recs = vec![
            SftrMarginActivityRecord {
                event_date: Some(today),
                ..Default::default()
            },
            SftrMarginActivityRecord {
                event_date: Some(NaiveDate::from_ymd_opt(2026, 5, 12).unwrap()),
                ..Default::default()
            },
        ];
        let out = SftrMarEventDateInFuture.run(&recs, &ctx_on(today));
        assert!(out.is_empty());
    }

    #[test]
    fn skips_records_without_event_date_eg_err_wrapper() {
        let r = SftrMarginActivityRecord {
            action_type: Some("ERRT".into()),
            event_date: None,
            ..Default::default()
        };
        let out = SftrMarEventDateInFuture
            .run(&[r], &ctx_on(NaiveDate::from_ymd_opt(2026, 5, 13).unwrap()));
        assert!(out.is_empty());
    }
}
