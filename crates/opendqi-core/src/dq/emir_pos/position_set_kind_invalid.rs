//! `EMIR.POS.POSITION_SET_KIND_INVALID` — the position-set
//! record carries no `position_set_kind` or a value outside
//! the closed enum of 4 wrapper codes (`PosSet`, `CcyPosSet`,
//! `CollPosSet`, `CcyCollPosSet`).
//!
//! The auth.090 parser only emits these 4 strings (derived
//! from the wrapper element names under `Rpt`). A record
//! reaching the engine with `None` or anything else means
//! either upstream malformed XML or a synthetic record that
//! bypassed the parser — either way the record cannot be
//! routed to the right downstream DQI computer (each computer
//! scopes by kind).

use super::EmirPosCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, EmirPositionSetRecord, Regime, Severity};

/// Check implementation.
pub struct EmirPosPositionSetKindInvalid;

const CHECK_ID: &str = "EMIR.POS.POSITION_SET_KIND_INVALID";
const VALID_KINDS: &[&str] = &["PosSet", "CcyPosSet", "CollPosSet", "CcyCollPosSet"];

fn is_valid(k: &str) -> bool {
    VALID_KINDS.contains(&k)
}

impl EmirPosCheck for EmirPosPositionSetKindInvalid {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Validity
    }
    fn severity(&self) -> Severity {
        Severity::Critical
    }
    fn run(&self, records: &[EmirPositionSetRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        let mut out = Vec::new();
        for r in records {
            let raised = match r.position_set_kind.as_deref() {
                None => Some("<none>".to_string()),
                Some(k) if !is_valid(k) => Some(k.to_string()),
                _ => None,
            };
            if let Some(observed) = raised {
                out.push(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Emir,
                    severity: Severity::Critical,
                    dimension: DqDimension::Validity,
                    record_id: r.record_id.clone(),
                    uti: None,
                    field: Some("position_set_kind".into()),
                    value: Some(observed.clone()),
                    message: format!(
                        "position_set_kind {observed:?} is not one of the 4 auth.090 wrapper \
                         element names (PosSet / CcyPosSet / CollPosSet / CcyCollPosSet); \
                         the record cannot be routed to the right downstream DQI computer"
                    ),
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
    fn fires_when_kind_is_none() {
        let r = EmirPositionSetRecord {
            record_id: Some("R-NONE".into()),
            position_set_kind: None,
            ..Default::default()
        };
        let out = EmirPosPositionSetKindInvalid.run(&[r], &ctx());
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn fires_on_unknown_kind() {
        let r = EmirPositionSetRecord {
            position_set_kind: Some("BOGUS".into()),
            ..Default::default()
        };
        let out = EmirPosPositionSetKindInvalid.run(&[r], &ctx());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].value.as_deref(), Some("BOGUS"));
    }

    #[test]
    fn does_not_fire_on_each_of_4_valid_kinds() {
        for k in VALID_KINDS {
            let r = EmirPositionSetRecord {
                position_set_kind: Some((*k).to_string()),
                ..Default::default()
            };
            let out = EmirPosPositionSetKindInvalid.run(&[r], &ctx());
            assert!(out.is_empty(), "{k} should be accepted");
        }
    }
}
