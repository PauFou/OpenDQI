//! EMIR.MAR.LARGE_MARGIN_DELTA — within the same portfolio, two
//! consecutive records show an IM or VM posted variation > 50% relative
//! to the prior amount.

use rust_decimal::Decimal;
use std::collections::BTreeMap;

use super::MarginActivityCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, MarginActivityRecord, Regime, Severity};

/// Check implementation.
pub struct EmirMarLargeMarginDelta;

const CHECK_ID: &str = "EMIR.MAR.LARGE_MARGIN_DELTA";
const RATIO_THRESHOLD: f64 = 0.50;

impl MarginActivityCheck for EmirMarLargeMarginDelta {
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
        records: &[MarginActivityRecord],
        _prior: &[MarginActivityRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        let mut by_portfolio: BTreeMap<&str, Vec<&MarginActivityRecord>> = BTreeMap::new();
        for r in records {
            if let Some(pc) = r.collateral_portfolio_code.as_deref() {
                by_portfolio.entry(pc).or_default().push(r);
            }
        }
        let mut out = Vec::new();
        for (portfolio, mut bucket) in by_portfolio {
            if bucket.len() < 2 {
                continue;
            }
            bucket.sort_by_key(|r| r.event_timestamp);
            for win in bucket.windows(2) {
                let (a, b) = (win[0], win[1]);
                for (field, av, bv) in [
                    (
                        "initial_margin_posted",
                        a.initial_margin_posted,
                        b.initial_margin_posted,
                    ),
                    (
                        "variation_margin_posted",
                        a.variation_margin_posted,
                        b.variation_margin_posted,
                    ),
                ] {
                    if let (Some(av), Some(bv)) = (av, bv) {
                        if av == Decimal::ZERO {
                            continue;
                        }
                        let af = av.to_string().parse::<f64>().unwrap_or(f64::NAN);
                        let bf = bv.to_string().parse::<f64>().unwrap_or(f64::NAN);
                        if !af.is_finite() || !bf.is_finite() || af == 0.0 {
                            continue;
                        }
                        let rel = (bf - af).abs() / af.abs();
                        if rel > RATIO_THRESHOLD {
                            out.push(DqIssue {
                                check_id: CHECK_ID.into(),
                                regime: Regime::Emir,
                                severity: Severity::Warning,
                                dimension: DqDimension::Accuracy,
                                record_id: b.record_id.clone(),
                                uti: b.uti.clone(),
                                field: Some(field.into()),
                                value: Some(format!("prev={av} curr={bv}")),
                                message: format!(
                                    "{field} on portfolio {portfolio} jumped {:.0}% (from {av} to {bv}).",
                                    rel * 100.0
                                ),
                                source_file: b.source_file.clone(),
                                evidence: Vec::new(),
                            });
                        }
                    }
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

    fn ts(s: &str) -> Option<DateTime<Utc>> {
        Some(
            chrono::DateTime::parse_from_rfc3339(s)
                .unwrap()
                .with_timezone(&Utc),
        )
    }

    #[test]
    fn flags_big_jump() {
        let a = MarginActivityRecord {
            collateral_portfolio_code: Some("P".into()),
            initial_margin_posted: Some(Decimal::from(100)),
            event_timestamp: ts("2026-05-12T08:00:00Z"),
            ..Default::default()
        };
        let b = MarginActivityRecord {
            collateral_portfolio_code: Some("P".into()),
            initial_margin_posted: Some(Decimal::from(500)),
            event_timestamp: ts("2026-05-13T08:00:00Z"),
            ..Default::default()
        };
        let out = EmirMarLargeMarginDelta.run(&[a, b], &[], &ctx());
        assert!(out.iter().any(|i| i.check_id == CHECK_ID));
    }

    #[test]
    fn accepts_small_change() {
        let a = MarginActivityRecord {
            collateral_portfolio_code: Some("P".into()),
            initial_margin_posted: Some(Decimal::from(100)),
            event_timestamp: ts("2026-05-12T08:00:00Z"),
            ..Default::default()
        };
        let b = MarginActivityRecord {
            collateral_portfolio_code: Some("P".into()),
            initial_margin_posted: Some(Decimal::from(110)),
            event_timestamp: ts("2026-05-13T08:00:00Z"),
            ..Default::default()
        };
        let out = EmirMarLargeMarginDelta.run(&[a, b], &[], &ctx());
        assert!(out.is_empty());
    }
}
