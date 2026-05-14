//! EMIR.LFC.VALUATION_AFTER_TERMINATION — a VALU action on a UTI
//! that has already been ETRM'd (prior termination_date <= current
//! valuation_timestamp.date()).

use std::collections::HashMap;

use chrono::NaiveDate;

use crate::dq::{CheckContext, LifecycleCheck};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct LifecycleValuationAfterTermination;

const CHECK_ID: &str = "EMIR.LFC.VALUATION_AFTER_TERMINATION";

impl LifecycleCheck for LifecycleValuationAfterTermination {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(
        &self,
        current: &[EmirRecord],
        prior: &[EmirRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        // Index prior ETRMs: smallest termination_date per UTI.
        let mut earliest_term: HashMap<&str, NaiveDate> = HashMap::new();
        for r in prior {
            let is_etrm = r
                .action_type
                .as_deref()
                .map(|a| a.eq_ignore_ascii_case("ETRM"))
                .unwrap_or(false);
            if !is_etrm {
                continue;
            }
            if let (Some(uti), Some(td)) = (r.uti.as_deref(), r.termination_date) {
                let key = uti.trim();
                earliest_term
                    .entry(key)
                    .and_modify(|cur| {
                        if td < *cur {
                            *cur = td;
                        }
                    })
                    .or_insert(td);
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
            let val_ts = match r.valuation_timestamp {
                Some(t) => t,
                None => continue,
            };
            if let Some(td) = earliest_term.get(uti) {
                if *td <= val_ts.date_naive() {
                    out.push(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Emir,
                        severity: Severity::High,
                        dimension: DqDimension::Consistency,
                        record_id: r.record_id.clone(),
                        uti: Some(uti.to_owned()),
                        field: Some("valuation_timestamp".into()),
                        value: Some(val_ts.to_rfc3339()),
                        message: format!(
                            "VALU for UTI {uti} on {ts} but prior ETRM termination_date is {td}.",
                            ts = val_ts.to_rfc3339(),
                            td = td,
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
    use chrono::{TimeZone, Utc};
    #[test]
    fn flags_valuation_after_etrm() {
        let current = vec![EmirRecord {
            uti: Some("U1".into()),
            action_type: Some("VALU".into()),
            valuation_timestamp: Some(Utc.with_ymd_and_hms(2026, 5, 10, 12, 0, 0).unwrap()),
            ..Default::default()
        }];
        let prior = vec![EmirRecord {
            uti: Some("U1".into()),
            action_type: Some("ETRM".into()),
            termination_date: NaiveDate::from_ymd_opt(2026, 5, 1),
            ..Default::default()
        }];
        let issues = LifecycleValuationAfterTermination.run(
            &current,
            &prior,
            &CheckContext::now_with_defaults(),
        );
        assert_eq!(issues.len(), 1);
    }
    #[test]
    fn accepts_valuation_before_etrm() {
        let current = vec![EmirRecord {
            uti: Some("U1".into()),
            action_type: Some("VALU".into()),
            valuation_timestamp: Some(Utc.with_ymd_and_hms(2026, 4, 20, 12, 0, 0).unwrap()),
            ..Default::default()
        }];
        let prior = vec![EmirRecord {
            uti: Some("U1".into()),
            action_type: Some("ETRM".into()),
            termination_date: NaiveDate::from_ymd_opt(2026, 5, 1),
            ..Default::default()
        }];
        let issues = LifecycleValuationAfterTermination.run(
            &current,
            &prior,
            &CheckContext::now_with_defaults(),
        );
        assert!(issues.is_empty());
    }
}
