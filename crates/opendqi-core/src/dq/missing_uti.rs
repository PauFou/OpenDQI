//! EMIR.COMP.UTI_MISSING — every record must carry a non-empty UTI.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct MissingUti;

const CHECK_ID: &str = "EMIR.COMP.UTI_MISSING";

impl Check for MissingUti {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| r.uti.as_deref().map(str::trim).unwrap_or("").is_empty())
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Emir,
                severity: Severity::High,
                dimension: DqDimension::Completeness,
                record_id: r.record_id.clone(),
                uti: None,
                field: Some("uti".into()),
                value: None,
                message: "UTI is missing or empty.".into(),
                source_file: r.source_file.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dq::CheckContext;

    #[test]
    fn flags_records_without_uti() {
        let records = vec![
            EmirRecord {
                uti: Some("OPENDQI-OK-1".into()),
                ..Default::default()
            },
            EmirRecord {
                uti: None,
                record_id: Some("2".into()),
                ..Default::default()
            },
            EmirRecord {
                uti: Some("   ".into()),
                record_id: Some("3".into()),
                ..Default::default()
            },
        ];
        let ctx = CheckContext::now_with_defaults();
        let issues = MissingUti.run(&records, &ctx);
        assert_eq!(issues.len(), 2);
        assert!(issues.iter().all(|i| i.check_id == "EMIR.COMP.UTI_MISSING"));
    }
}
