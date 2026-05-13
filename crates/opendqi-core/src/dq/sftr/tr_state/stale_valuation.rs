//! SFTR.TST.STALE_VALUATION — TR-held collateral valuation is older
//! than the configured threshold relative to the TSR's `state_as_of`.
//! Approximates "valuation" by `state_as_of` itself : SFT TSR
//! snapshots are dated as of the report header, so a TSR header
//! older than the threshold means the snapshot itself is stale.

use chrono::Duration;

use super::{is_outstanding, SftrTrStateCheck};
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord, SftrTrStateRecord};

/// Check implementation.
pub struct SftrStaleValuationOnTsr;

const CHECK_ID: &str = "SFTR.TST.STALE_VALUATION";

fn business_days_to_calendar(d: i64) -> i64 {
    ((d as f64) * 1.4).ceil() as i64
}

impl SftrTrStateCheck for SftrStaleValuationOnTsr {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
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
        let max_age =
            business_days_to_calendar(ctx.thresholds.timeliness.max_valuation_age_business_days);
        records
            .iter()
            .filter_map(|r| {
                if !is_outstanding(r) {
                    return None;
                }
                let state_as_of = r.state_as_of?;
                let age = ctx.now.signed_duration_since(state_as_of);
                if age <= Duration::days(max_age) {
                    return None;
                }
                Some(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Sftr,
                    severity: Severity::High,
                    dimension: DqDimension::Accuracy,
                    record_id: r.record_id.clone(),
                    uti: r.uti.clone(),
                    field: Some("state_as_of".into()),
                    value: Some(state_as_of.to_rfc3339()),
                    message: format!(
                        "TSR snapshot (state_as_of {ts}) is older than {max_age} calendar days vs ctx.now.",
                        ts = state_as_of.to_rfc3339(),
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
    use chrono::{TimeZone, Utc};
    #[test]
    fn flags_old_snapshot() {
        let r = SftrTrStateRecord {
            uti: Some("U1".into()),
            status: Some("OUTSTANDING".into()),
            state_as_of: Some(Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap()),
            ..Default::default()
        };
        assert_eq!(
            SftrStaleValuationOnTsr
                .run(&[r], &[], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_fresh_snapshot() {
        let r = SftrTrStateRecord {
            uti: Some("U1".into()),
            status: Some("OUTSTANDING".into()),
            state_as_of: Some(Utc::now()),
            ..Default::default()
        };
        assert!(SftrStaleValuationOnTsr
            .run(&[r], &[], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
