//! EMIR.TST.STALE_VALUATION — the latest valuation the TR is holding
//! is older than the configured staleness threshold compared to the
//! TSR's `state_as_of` timestamp (or `ctx.today` if no header
//! timestamp is available).

use chrono::Duration;

use super::is_outstanding;
use crate::dq::{CheckContext, TrStateCheck};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity, TrStateRecord};

/// Check implementation.
pub struct EmirStaleValuationOnTsr;

const CHECK_ID: &str = "EMIR.TST.STALE_VALUATION";

/// Approximate business-day → calendar-day conversion for the
/// threshold lookup. A more accurate calendar can be plugged later.
fn business_days_to_calendar(d: i64) -> i64 {
    // 1 business day ≈ 1.4 calendar days (rough weekend slack).
    ((d as f64) * 1.4).ceil() as i64
}

impl TrStateCheck for EmirStaleValuationOnTsr {
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
        records: &[TrStateRecord],
        _prior: &[EmirRecord],
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
                let val_ts = r.valuation_timestamp?;
                let reference = r.state_as_of.unwrap_or(ctx.now);
                let age = reference.signed_duration_since(val_ts);
                if age <= Duration::days(max_age) {
                    return None;
                }
                Some(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Emir,
                    severity: Severity::High,
                    dimension: DqDimension::Accuracy,
                    record_id: r.record_id.clone(),
                    uti: r.uti.clone(),
                    field: Some("valuation_timestamp".into()),
                    value: Some(val_ts.to_rfc3339()),
                    message: format!(
                        "TR-held valuation timestamp {ts} is older than the {max_age}-day threshold relative to the TSR state-as-of {ref}.",
                        ts = val_ts.to_rfc3339(),
                        ref = reference.to_rfc3339(),
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
    fn flags_stale_valuation() {
        let rec = TrStateRecord {
            uti: Some("U1".into()),
            status: Some("OUTSTANDING".into()),
            valuation_timestamp: Some(Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap()),
            state_as_of: Some(Utc.with_ymd_and_hms(2026, 5, 13, 8, 0, 0).unwrap()),
            ..Default::default()
        };
        assert_eq!(
            EmirStaleValuationOnTsr
                .run(&[rec], &[], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }

    #[test]
    fn ignores_fresh_valuation() {
        let rec = TrStateRecord {
            uti: Some("U1".into()),
            status: Some("OUTSTANDING".into()),
            valuation_timestamp: Some(Utc.with_ymd_and_hms(2026, 5, 13, 7, 0, 0).unwrap()),
            state_as_of: Some(Utc.with_ymd_and_hms(2026, 5, 13, 8, 0, 0).unwrap()),
            ..Default::default()
        };
        assert!(EmirStaleValuationOnTsr
            .run(&[rec], &[], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
