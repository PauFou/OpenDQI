//! SFTR.TST.ACTIVE_PAST_MATURITY — TSR shows the SFT as outstanding
//! but its `maturity_date` is in the past.

use super::{is_outstanding, SftrTrStateCheck};
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord, SftrTrStateRecord};

/// Check implementation.
pub struct SftrActivePastMaturity;

const CHECK_ID: &str = "SFTR.TST.ACTIVE_PAST_MATURITY";

impl SftrTrStateCheck for SftrActivePastMaturity {
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
        records: &[SftrTrStateRecord],
        _prior: &[SftrRecord],
        ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        records
            .iter()
            .filter_map(|r| {
                if !is_outstanding(r) || r.termination_date.is_some() {
                    return None;
                }
                let maturity = r.maturity_date?;
                let reference = r.state_as_of.map(|t| t.date_naive()).unwrap_or(ctx.today);
                if maturity > reference {
                    return None;
                }
                Some(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Sftr,
                    severity: Severity::High,
                    dimension: DqDimension::Consistency,
                    record_id: r.record_id.clone(),
                    uti: r.uti.clone(),
                    field: Some("maturity_date".into()),
                    value: Some(maturity.to_string()),
                    message: format!(
                        "TR shows SFT as outstanding on {reference} but maturity is {maturity}."
                    ),
                    source_file: r.source_file.clone(),
                    evidence: Vec::new(),
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
        let r = SftrTrStateRecord {
            uti: Some("U1".into()),
            status: Some("OUTSTANDING".into()),
            maturity_date: NaiveDate::from_ymd_opt(2020, 1, 1),
            state_as_of: Some(Utc.with_ymd_and_hms(2026, 5, 13, 0, 0, 0).unwrap()),
            ..Default::default()
        };
        assert_eq!(
            SftrActivePastMaturity
                .run(&[r], &[], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_future_maturity() {
        let r = SftrTrStateRecord {
            uti: Some("U1".into()),
            status: Some("OUTSTANDING".into()),
            maturity_date: NaiveDate::from_ymd_opt(2099, 1, 1),
            state_as_of: Some(Utc.with_ymd_and_hms(2026, 5, 13, 0, 0, 0).unwrap()),
            ..Default::default()
        };
        assert!(SftrActivePastMaturity
            .run(&[r], &[], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
