//! `EMIR.ACK.ENVELOPE_WELLFORMED` — the auth.031 ack record is
//! missing the minimum identifying information.
//!
//! A Status Advice has no operational meaning unless it carries:
//!   1. `submission_id` (so the firm can correlate the ack with
//!      the original submission it sent)
//!   2. `ack_status` (the verdict — ACPT/ACTC/RJCT/PDNG/PRTL)
//!   3. `ack_timestamp` (so SLA + timeliness can be measured)
//!
//! When any of the three is missing the check raises a Critical
//! issue per record. The check covers the entire DQ surface for
//! auth.031 — there is no payload to validate beyond the envelope.

use super::EmirStatusAdviceCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, EmirStatusAdviceRecord, Regime, Severity};

/// Check implementation.
pub struct EmirStatusAdviceEnvelopeWellformed;

const CHECK_ID: &str = "EMIR.ACK.ENVELOPE_WELLFORMED";

impl EmirStatusAdviceCheck for EmirStatusAdviceEnvelopeWellformed {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Validity
    }
    fn severity(&self) -> Severity {
        Severity::Critical
    }
    fn run(&self, records: &[EmirStatusAdviceRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        let mut out = Vec::new();
        for r in records {
            let missing: Vec<&str> = [
                ("submission_id", r.submission_id.is_none()),
                ("ack_status", r.ack_status.is_none()),
                ("ack_timestamp", r.ack_timestamp.is_none()),
            ]
            .into_iter()
            .filter_map(|(k, miss)| if miss { Some(k) } else { None })
            .collect();
            if !missing.is_empty() {
                let joined = missing.join(", ");
                out.push(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Emir,
                    severity: Severity::Critical,
                    dimension: DqDimension::Validity,
                    record_id: r.record_id.clone(),
                    uti: None,
                    field: Some(joined.clone()),
                    value: None,
                    message: format!("auth.031 ack envelope is missing: {joined}"),
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

    fn ctx() -> CheckContext {
        CheckContext {
            thresholds: Default::default(),
            today: chrono::NaiveDate::from_ymd_opt(2026, 5, 21).unwrap(),
            now: chrono::DateTime::parse_from_rfc3339("2026-05-21T00:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
        }
    }

    #[test]
    fn passes_when_all_three_fields_present() {
        let r = EmirStatusAdviceRecord {
            record_id: Some("R1".into()),
            submission_id: Some("SUB-X".into()),
            ack_status: Some("ACPT".into()),
            ack_timestamp: Some(chrono::Utc::now()),
            ..Default::default()
        };
        let out = EmirStatusAdviceEnvelopeWellformed.run(&[r], &ctx());
        assert!(out.is_empty());
    }

    #[test]
    fn fires_when_ack_status_missing() {
        let r = EmirStatusAdviceRecord {
            record_id: Some("R1".into()),
            submission_id: Some("SUB-X".into()),
            ack_status: None,
            ack_timestamp: Some(chrono::Utc::now()),
            ..Default::default()
        };
        let out = EmirStatusAdviceEnvelopeWellformed.run(&[r], &ctx());
        assert_eq!(out.len(), 1);
        assert!(out[0].field.as_deref().unwrap().contains("ack_status"));
    }

    #[test]
    fn fires_listing_all_missing_fields() {
        let r = EmirStatusAdviceRecord {
            record_id: Some("R1".into()),
            ..Default::default()
        };
        let out = EmirStatusAdviceEnvelopeWellformed.run(&[r], &ctx());
        assert_eq!(out.len(), 1);
        let field = out[0].field.as_deref().unwrap();
        assert!(field.contains("submission_id"));
        assert!(field.contains("ack_status"));
        assert!(field.contains("ack_timestamp"));
    }
}
