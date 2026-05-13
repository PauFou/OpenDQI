//! EMIR.TST.ACTIVE_PAST_MATURITY — TSR shows a trade as outstanding
//! but its `maturity_date` is already in the past (relative to the
//! TSR's `state_as_of` or `ctx.today` fallback).

use super::is_outstanding;
use crate::dq::{CheckContext, TrStateCheck};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity, TrStateRecord};

/// Check implementation.
pub struct EmirActivePastMaturity;

const CHECK_ID: &str = "EMIR.TST.ACTIVE_PAST_MATURITY";

impl TrStateCheck for EmirActivePastMaturity {
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
        records: &[TrStateRecord],
        _prior: &[EmirRecord],
        ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        records
            .iter()
            .filter_map(|r| {
                if !is_outstanding(r) || r.termination_date.is_some() {
                    return None;
                }
                let maturity = r.maturity_date?;
                let reference = r
                    .state_as_of
                    .map(|t| t.date_naive())
                    .unwrap_or(ctx.today);
                if maturity > reference {
                    return None;
                }
                Some(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Emir,
                    severity: Severity::High,
                    dimension: DqDimension::Consistency,
                    record_id: r.record_id.clone(),
                    uti: r.uti.clone(),
                    field: Some("maturity_date".into()),
                    value: Some(maturity.to_string()),
                    message: format!(
                        "TR shows trade as outstanding on {reference} but its maturity date is {maturity}."
                    ),
                    source_file: r.source_file.clone(),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, TimeZone, Utc};

    #[test]
    fn flags_past_maturity() {
        let rec = TrStateRecord {
            uti: Some("U1".into()),
            status: Some("OUTSTANDING".into()),
            maturity_date: NaiveDate::from_ymd_opt(2026, 1, 1),
            state_as_of: Some(Utc.with_ymd_and_hms(2026, 5, 13, 8, 0, 0).unwrap()),
            ..Default::default()
        };
        assert_eq!(
            EmirActivePastMaturity
                .run(&[rec], &[], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }

    #[test]
    fn ignores_future_maturity() {
        let rec = TrStateRecord {
            uti: Some("U1".into()),
            status: Some("OUTSTANDING".into()),
            maturity_date: NaiveDate::from_ymd_opt(2030, 1, 1),
            state_as_of: Some(Utc.with_ymd_and_hms(2026, 5, 13, 8, 0, 0).unwrap()),
            ..Default::default()
        };
        assert!(EmirActivePastMaturity
            .run(&[rec], &[], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
