//! EMIR.VLD.EVENT_TYPE_ENUM — event type must be one of the
//! ESMA-defined codes.

use super::{Check, CheckContext};
use crate::dq::formats::is_in;
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct EventTypeEnum;

const CHECK_ID: &str = "EMIR.VLD.EVENT_TYPE_ENUM";
const ALLOWED: &[&str] = &[
    "TRAD", "NOVA", "INCP", "COMP", "ETRM", "EXER", "ALOC", "CLRG", "CORP", "EXPI", "REVI", "UPDT",
    "INCR", "PTNG", "CREA",
];

impl Check for EventTypeEnum {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Validity
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter_map(|r| {
                let v = r.event_type.as_deref()?.trim();
                if v.is_empty() || is_in(v, ALLOWED) {
                    None
                } else {
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Emir,
                        severity: Severity::Warning,
                        dimension: DqDimension::Validity,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("event_type".into()),
                        value: Some(v.to_owned()),
                        message: format!(
                            "Event type '{v}' is not in the ESMA-defined enumeration."
                        ),
                        source_file: r.source_file.clone(),
                        evidence: Vec::new(),
                    })
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flags_unknown_event() {
        let r = EmirRecord {
            event_type: Some("BOGUS".into()),
            ..Default::default()
        };
        assert_eq!(
            EventTypeEnum
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn accepts_known_event() {
        let r = EmirRecord {
            event_type: Some("TRAD".into()),
            ..Default::default()
        };
        assert!(EventTypeEnum
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
