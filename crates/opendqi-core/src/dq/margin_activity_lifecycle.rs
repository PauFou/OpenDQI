//! Cross-batch lifecycle checks for the EMIR MAR (`auth.108`).
//! Re-uses the existing `MarginActivityCheck` trait (which already
//! accepts a `prior: &[MarginActivityRecord]` parameter).

use std::collections::{BTreeMap, HashSet};

use chrono::Duration;
use rust_decimal::Decimal;

use crate::dq::margin_activity::MarginActivityCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, MarginActivityRecord, Regime, Severity};

#[allow(clippy::too_many_arguments)]
fn issue(
    check_id: &str,
    severity: Severity,
    dimension: DqDimension,
    portfolio: &str,
    record_id: Option<String>,
    field: &str,
    value: Option<String>,
    message: String,
    source_file: Option<String>,
) -> DqIssue {
    DqIssue {
        check_id: check_id.into(),
        regime: Regime::Emir,
        severity,
        dimension,
        record_id,
        uti: None,
        field: Some(field.into()),
        value,
        message: format!("[portfolio={portfolio}] {message}"),
        source_file,
    }
}

/// `EMIR.MAR.LFC.PORTFOLIO_GAP` — portfolio_code present in prior
/// snapshot has no event in the current batch.
pub struct EmirMarLfcPortfolioGap;

impl MarginActivityCheck for EmirMarLfcPortfolioGap {
    fn id(&self) -> &'static str {
        "EMIR.MAR.LFC.PORTFOLIO_GAP"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(
        &self,
        records: &[MarginActivityRecord],
        prior: &[MarginActivityRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        let current_portfolios: HashSet<&str> = records
            .iter()
            .filter_map(|r| r.collateral_portfolio_code.as_deref())
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .collect();
        let prior_portfolios: HashSet<&str> = prior
            .iter()
            .filter_map(|r| r.collateral_portfolio_code.as_deref())
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .collect();
        let mut out = Vec::new();
        for pc in prior_portfolios.difference(&current_portfolios) {
            out.push(issue(
                self.id(),
                self.severity(),
                self.dimension(),
                pc,
                None,
                "collateral_portfolio_code",
                None,
                "no margin event in the current batch.".into(),
                None,
            ));
        }
        out
    }
}

/// `EMIR.MAR.LFC.RECURRING_LATE_MARGIN` — the same portfolio shows
/// `reporting_timestamp - event_timestamp > 24h` in both the prior
/// and the current snapshot.
pub struct EmirMarLfcRecurringLateMargin;

impl MarginActivityCheck for EmirMarLfcRecurringLateMargin {
    fn id(&self) -> &'static str {
        "EMIR.MAR.LFC.RECURRING_LATE_MARGIN"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Timeliness
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(
        &self,
        records: &[MarginActivityRecord],
        prior: &[MarginActivityRecord],
        ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        let max = Duration::hours(ctx.thresholds.timeliness.max_reporting_delay_hours);
        let is_late = |r: &MarginActivityRecord| -> bool {
            match (r.event_timestamp, r.reporting_timestamp) {
                (Some(ev), Some(rep)) => rep > ev && rep - ev > max,
                _ => false,
            }
        };
        let prior_late: HashSet<&str> = prior
            .iter()
            .filter(|r| is_late(r))
            .filter_map(|r| r.collateral_portfolio_code.as_deref())
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .collect();
        let mut out = Vec::new();
        let mut emitted: HashSet<String> = HashSet::new();
        for r in records.iter().filter(|r| is_late(r)) {
            let Some(pc) = r
                .collateral_portfolio_code
                .as_deref()
                .map(str::trim)
                .filter(|p| !p.is_empty())
            else {
                continue;
            };
            if !prior_late.contains(pc) {
                continue;
            }
            if !emitted.insert(pc.to_owned()) {
                continue;
            }
            out.push(issue(
                self.id(),
                self.severity(),
                self.dimension(),
                pc,
                r.record_id.clone(),
                "reporting_timestamp",
                None,
                "late margin reporting recurs across two consecutive batches.".into(),
                r.source_file.clone(),
            ));
        }
        out
    }
}

/// `EMIR.MAR.LFC.NEGATIVE_TREND` — the same portfolio shows a
/// strictly decreasing IM or VM posted across ≥3 snapshots (prior +
/// current). v1 uses the maximum IM posted per portfolio in each
/// scan as the comparison point.
pub struct EmirMarLfcNegativeTrend;

impl MarginActivityCheck for EmirMarLfcNegativeTrend {
    fn id(&self) -> &'static str {
        "EMIR.MAR.LFC.NEGATIVE_TREND"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(
        &self,
        records: &[MarginActivityRecord],
        prior: &[MarginActivityRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        // For v1 we approximate a "trend" with two points: the max IM
        // posted observed in `prior` and the max in `current`. We
        // emit when current's max < prior's max (true regression).
        // Detecting a 3-point monotone trend would require either
        // batch-tagged prior rows or persisted scan_ids — defer that
        // refinement until the store loader exposes scan_id.
        fn max_im_by_portfolio(rs: &[MarginActivityRecord]) -> BTreeMap<String, Decimal> {
            let mut acc: BTreeMap<String, Decimal> = BTreeMap::new();
            for r in rs {
                let Some(pc) = r.collateral_portfolio_code.as_deref().map(str::trim) else {
                    continue;
                };
                if pc.is_empty() {
                    continue;
                }
                if let Some(im) = r.initial_margin_posted {
                    acc.entry(pc.to_owned())
                        .and_modify(|m| {
                            if im > *m {
                                *m = im;
                            }
                        })
                        .or_insert(im);
                }
            }
            acc
        }
        let prior_max = max_im_by_portfolio(prior);
        let curr_max = max_im_by_portfolio(records);
        let mut out = Vec::new();
        for (pc, curr) in &curr_max {
            let Some(prev) = prior_max.get(pc) else {
                continue;
            };
            if curr < prev {
                out.push(issue(
                    self.id(),
                    self.severity(),
                    self.dimension(),
                    pc,
                    None,
                    "initial_margin_posted",
                    Some(format!("prev_max={prev} curr_max={curr}")),
                    format!("initial_margin_posted is decreasing (prior max {prev}, current max {curr})."),
                    None,
                ));
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
            today: chrono::NaiveDate::from_ymd_opt(2026, 5, 14).unwrap(),
            now: chrono::DateTime::parse_from_rfc3339("2026-05-14T08:00:00Z")
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
    fn portfolio_gap_flags_and_accepts() {
        let prior = vec![MarginActivityRecord {
            collateral_portfolio_code: Some("PORT-A".into()),
            ..Default::default()
        }];
        let current = vec![MarginActivityRecord {
            collateral_portfolio_code: Some("PORT-B".into()),
            ..Default::default()
        }];
        let out = EmirMarLfcPortfolioGap.run(&current, &prior, &ctx());
        assert_eq!(out.len(), 1);
        // Same portfolio in both → no gap.
        let out2 = EmirMarLfcPortfolioGap.run(&prior, &prior, &ctx());
        assert!(out2.is_empty());
    }

    #[test]
    fn recurring_late_margin_flags_and_accepts() {
        let late = MarginActivityRecord {
            collateral_portfolio_code: Some("PORT-A".into()),
            event_timestamp: ts("2026-05-10T08:00:00Z"),
            reporting_timestamp: ts("2026-05-13T08:00:00Z"),
            ..Default::default()
        };
        let out = EmirMarLfcRecurringLateMargin.run(
            std::slice::from_ref(&late),
            std::slice::from_ref(&late),
            &ctx(),
        );
        assert_eq!(out.len(), 1);
        // Only late in current — not "recurring".
        let on_time = MarginActivityRecord {
            collateral_portfolio_code: Some("PORT-A".into()),
            event_timestamp: ts("2026-05-13T07:00:00Z"),
            reporting_timestamp: ts("2026-05-13T08:00:00Z"),
            ..Default::default()
        };
        let out2 = EmirMarLfcRecurringLateMargin.run(&[late], &[on_time], &ctx());
        assert!(out2.is_empty());
    }

    #[test]
    fn negative_trend_flags_and_accepts() {
        let prior = vec![MarginActivityRecord {
            collateral_portfolio_code: Some("PORT-A".into()),
            initial_margin_posted: Some(Decimal::from(1_000_000)),
            ..Default::default()
        }];
        let current = vec![MarginActivityRecord {
            collateral_portfolio_code: Some("PORT-A".into()),
            initial_margin_posted: Some(Decimal::from(500_000)),
            ..Default::default()
        }];
        assert_eq!(
            EmirMarLfcNegativeTrend.run(&current, &prior, &ctx()).len(),
            1
        );
        // Increasing → accept.
        let current2 = vec![MarginActivityRecord {
            collateral_portfolio_code: Some("PORT-A".into()),
            initial_margin_posted: Some(Decimal::from(1_500_000)),
            ..Default::default()
        }];
        assert!(EmirMarLfcNegativeTrend
            .run(&current2, &prior, &ctx())
            .is_empty());
    }
}
