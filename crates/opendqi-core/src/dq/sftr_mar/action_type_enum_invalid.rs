//! `SFTR.MAR.ACTION_TYPE_ENUM_INVALID` — the MAR `action_type`
//! is missing or outside the closed enum of 4 wrapper codes
//! (`NEWT`/`ERRT`/`CORR`/`TRDU`) derived from the auth.070
//! `TradeReport21Choice__1` wrappers (`New`/`Err`/`Crrctn`/
//! `TradUpd`).
//!
//! The parser maps the wrapper element name onto the canonical
//! 4-letter code, so a record reaching the engine with `None`
//! or anything else means either upstream malformed XML or a
//! synthetic / hand-constructed record that bypassed the parser.
//! Both are Critical — without a valid action type the event
//! cannot be interpreted (new vs error vs correction vs update).

use super::SftrMarCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrMarginActivityRecord};

/// Check implementation.
pub struct SftrMarActionTypeEnumInvalid;

const CHECK_ID: &str = "SFTR.MAR.ACTION_TYPE_ENUM_INVALID";

/// The 4 valid action_type codes the auth.070 parser emits.
const VALID_ACTION_TYPES: &[&str] = &["NEWT", "ERRT", "CORR", "TRDU"];

fn is_valid(code: &str) -> bool {
    VALID_ACTION_TYPES.contains(&code)
}

impl SftrMarCheck for SftrMarActionTypeEnumInvalid {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Validity
    }
    fn severity(&self) -> Severity {
        Severity::Critical
    }
    fn run(&self, records: &[SftrMarginActivityRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        let mut out = Vec::new();
        for r in records {
            let raised = match r.action_type.as_deref() {
                None => Some("<none>".to_string()),
                Some(c) if !is_valid(c) => Some(c.to_string()),
                _ => None,
            };
            if let Some(observed) = raised {
                out.push(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Sftr,
                    severity: Severity::Critical,
                    dimension: DqDimension::Validity,
                    record_id: r.record_id.clone(),
                    uti: r.collateral_portfolio_code.clone(),
                    field: Some("action_type".into()),
                    value: Some(observed.clone()),
                    message: format!(
                        "action_type {observed:?} is not one of the 4 auth.070 wrapper codes \
                         (NEWT/ERRT/CORR/TRDU); the event cannot be classified"
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
            today: chrono::NaiveDate::from_ymd_opt(2026, 5, 13).unwrap(),
            now: chrono::DateTime::parse_from_rfc3339("2026-05-13T08:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
        }
    }

    #[test]
    fn fires_when_action_type_is_none() {
        let r = SftrMarginActivityRecord {
            record_id: Some("R-NONE".into()),
            action_type: None,
            ..Default::default()
        };
        let out = SftrMarActionTypeEnumInvalid.run(&[r], &ctx());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].severity, Severity::Critical);
    }

    #[test]
    fn fires_on_unknown_code() {
        let r = SftrMarginActivityRecord {
            action_type: Some("BOGUS".into()),
            ..Default::default()
        };
        let out = SftrMarActionTypeEnumInvalid.run(&[r], &ctx());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].value.as_deref(), Some("BOGUS"));
    }

    #[test]
    fn does_not_fire_on_each_of_4_valid_codes() {
        for code in VALID_ACTION_TYPES {
            let r = SftrMarginActivityRecord {
                action_type: Some((*code).to_string()),
                ..Default::default()
            };
            let out = SftrMarActionTypeEnumInvalid.run(&[r], &ctx());
            assert!(out.is_empty(), "{code} should be accepted");
        }
    }
}
