//! `EMIR.QRY.ENVELOPE_WELLFORMED` — the auth.029 query record
//! is missing the minimum identifying information.
//!
//! A Trade Report Query has no regulatory meaning unless it
//! carries both:
//!   1. an opaque query identifier (so the TR + the firm can
//!      correlate the request and the eventual response)
//!   2. the LEI of the requesting firm (so the TR knows which
//!      derivatives book to filter to)
//!
//! When either is missing the parser raises a Critical issue
//! per record. The check covers the entire DQ surface for
//! auth.029 — there is no payload to validate beyond the
//! envelope itself.

use super::EmirQueryCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, EmirQueryRecord, Regime, Severity};

/// Check implementation.
pub struct EmirQueryEnvelopeWellformed;

const CHECK_ID: &str = "EMIR.QRY.ENVELOPE_WELLFORMED";

impl EmirQueryCheck for EmirQueryEnvelopeWellformed {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Validity
    }
    fn severity(&self) -> Severity {
        Severity::Critical
    }
    fn run(&self, records: &[EmirQueryRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        let mut out = Vec::new();
        for r in records {
            let missing: Vec<&str> = [
                ("query_id", r.query_id.is_none()),
                ("requesting_lei", r.requesting_lei.is_none()),
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
                    message: format!("auth.029 query envelope is missing: {joined}"),
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
    fn passes_when_both_query_id_and_requesting_lei_are_present() {
        let r = EmirQueryRecord {
            record_id: Some("R1".into()),
            query_id: Some("QRY-001".into()),
            requesting_lei: Some("549300ABCDEFGH123456".into()),
            ..Default::default()
        };
        let out = EmirQueryEnvelopeWellformed.run(&[r], &ctx());
        assert!(out.is_empty());
    }

    #[test]
    fn fires_when_query_id_is_missing() {
        let r = EmirQueryRecord {
            record_id: Some("R1".into()),
            query_id: None,
            requesting_lei: Some("549300ABCDEFGH123456".into()),
            ..Default::default()
        };
        let out = EmirQueryEnvelopeWellformed.run(&[r], &ctx());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].severity, Severity::Critical);
        assert!(out[0].field.as_deref().unwrap().contains("query_id"));
    }

    #[test]
    fn fires_when_both_fields_are_missing() {
        let r = EmirQueryRecord {
            record_id: Some("R1".into()),
            ..Default::default()
        };
        let out = EmirQueryEnvelopeWellformed.run(&[r], &ctx());
        assert_eq!(out.len(), 1);
        let field = out[0].field.as_deref().unwrap();
        assert!(field.contains("query_id"));
        assert!(field.contains("requesting_lei"));
    }
}
