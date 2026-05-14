//! EMIR.LFC.VALUATION_REGRESSION — a VALU action whose
//! `valuation_timestamp` is strictly before the latest known prior
//! VALU timestamp for the same UTI.

use std::collections::HashMap;

use chrono::{DateTime, Utc};

use crate::dq::{CheckContext, LifecycleCheck};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct ValuationRegression;

const CHECK_ID: &str = "EMIR.LFC.VALUATION_REGRESSION";

impl LifecycleCheck for ValuationRegression {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(
        &self,
        current: &[EmirRecord],
        prior: &[EmirRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        // Build per-UTI max valuation_timestamp from prior VALU rows.
        let mut max_prior: HashMap<&str, DateTime<Utc>> = HashMap::new();
        for r in prior {
            let is_valu = r
                .action_type
                .as_deref()
                .map(|a| a.eq_ignore_ascii_case("VALU"))
                .unwrap_or(false);
            if !is_valu {
                continue;
            }
            if let (Some(uti), Some(ts)) = (r.uti.as_deref(), r.valuation_timestamp) {
                let key = uti.trim();
                max_prior
                    .entry(key)
                    .and_modify(|cur| {
                        if ts > *cur {
                            *cur = ts;
                        }
                    })
                    .or_insert(ts);
            }
        }

        let mut out = Vec::new();
        for r in current {
            if r.action_type
                .as_deref()
                .map(|a| !a.eq_ignore_ascii_case("VALU"))
                .unwrap_or(true)
            {
                continue;
            }
            let uti = match r.uti.as_deref() {
                Some(u) if !u.trim().is_empty() => u.trim(),
                _ => continue,
            };
            let cur_ts = match r.valuation_timestamp {
                Some(t) => t,
                None => continue,
            };
            if let Some(prev_ts) = max_prior.get(uti) {
                if cur_ts < *prev_ts {
                    out.push(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Emir,
                        severity: Severity::Warning,
                        dimension: DqDimension::Consistency,
                        record_id: r.record_id.clone(),
                        uti: Some(uti.to_owned()),
                        field: Some("valuation_timestamp".into()),
                        value: Some(cur_ts.to_rfc3339()),
                        message: format!(
                            "VALU for UTI {uti} has valuation_timestamp {cur} earlier than the latest known prior VALU at {prev}.",
                            cur = cur_ts.to_rfc3339(),
                            prev = prev_ts.to_rfc3339(),
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
    use chrono::TimeZone;
    #[test]
    fn flags_regression() {
        let current = vec![EmirRecord {
            uti: Some("U1".into()),
            action_type: Some("VALU".into()),
            valuation_timestamp: Some(Utc.with_ymd_and_hms(2026, 4, 1, 10, 0, 0).unwrap()),
            ..Default::default()
        }];
        let prior = vec![EmirRecord {
            uti: Some("U1".into()),
            action_type: Some("VALU".into()),
            valuation_timestamp: Some(Utc.with_ymd_and_hms(2026, 4, 5, 10, 0, 0).unwrap()),
            ..Default::default()
        }];
        let issues = ValuationRegression.run(&current, &prior, &CheckContext::now_with_defaults());
        assert_eq!(issues.len(), 1);
    }
    #[test]
    fn accepts_progression() {
        let current = vec![EmirRecord {
            uti: Some("U1".into()),
            action_type: Some("VALU".into()),
            valuation_timestamp: Some(Utc.with_ymd_and_hms(2026, 4, 6, 10, 0, 0).unwrap()),
            ..Default::default()
        }];
        let prior = vec![EmirRecord {
            uti: Some("U1".into()),
            action_type: Some("VALU".into()),
            valuation_timestamp: Some(Utc.with_ymd_and_hms(2026, 4, 5, 10, 0, 0).unwrap()),
            ..Default::default()
        }];
        let issues = ValuationRegression.run(&current, &prior, &CheckContext::now_with_defaults());
        assert!(issues.is_empty());
    }
}
