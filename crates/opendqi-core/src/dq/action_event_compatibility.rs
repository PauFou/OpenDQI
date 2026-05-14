//! EMIR.CON.ACTION_EVENT_COMPATIBILITY — verify the reported
//! `event_type` is compatible with the `action_type` per the ESMA
//! action × event matrix. An empty `allowed` slice in the matrix
//! means "accept any event type" (e.g. `CORR` corrections override).

use super::{Check, CheckContext};
use crate::dq::formats::is_in;
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct ActionEventCompatibility;

const CHECK_ID: &str = "EMIR.CON.ACTION_EVENT_COMPATIBILITY";

/// Plausible interpretation of the ESMA EMIR Refit action × event
/// matrix. An empty allowed-events slice means "accept any value".
const ALLOWED_PAIRS: &[(&str, &[&str])] = &[
    ("NEWT", &["TRAD", "NOVA", "INCP"]),
    (
        "MODI",
        &["TRAD", "NOVA", "COMP", "PTNG", "CLRG", "UPDT", "CREV"],
    ),
    ("CORR", &[]),
    ("ETRM", &["ETRM", "UPDT"]),
    ("VALU", &["UPDT", "MODI"]),
    ("MARU", &["UPDT"]),
    ("POSC", &["COMP", "UPDT"]),
    ("OTHR", &[]),
];

impl Check for ActionEventCompatibility {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter_map(|r| {
                let action = r.action_type.as_deref()?.trim();
                let event = r.event_type.as_deref()?.trim();
                if action.is_empty() || event.is_empty() {
                    return None;
                }
                let allowed = ALLOWED_PAIRS
                    .iter()
                    .find(|(a, _)| a.eq_ignore_ascii_case(action))
                    .map(|(_, evs)| *evs)?;
                // Empty slice = accept anything.
                if allowed.is_empty() || is_in(event, allowed) {
                    return None;
                }
                Some(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Emir,
                    severity: Severity::High,
                    dimension: DqDimension::Consistency,
                    record_id: r.record_id.clone(),
                    uti: r.uti.clone(),
                    field: Some("event_type".into()),
                    value: Some(event.to_owned()),
                    message: format!(
                        "Event type '{event}' is not compatible with action '{action}'. Expected one of: {allowed:?}.",
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

    #[test]
    fn flags_incompatible_pair() {
        let r = EmirRecord {
            action_type: Some("NEWT".into()),
            event_type: Some("ETRM".into()),
            ..Default::default()
        };
        assert_eq!(
            ActionEventCompatibility
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }

    #[test]
    fn accepts_compatible_pair() {
        let r = EmirRecord {
            action_type: Some("NEWT".into()),
            event_type: Some("TRAD".into()),
            ..Default::default()
        };
        assert!(ActionEventCompatibility
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }

    #[test]
    fn corr_accepts_any_event() {
        let r = EmirRecord {
            action_type: Some("CORR".into()),
            event_type: Some("EXOTIC".into()),
            ..Default::default()
        };
        assert!(ActionEventCompatibility
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
