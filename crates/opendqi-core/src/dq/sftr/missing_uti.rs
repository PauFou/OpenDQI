//! SFTR.COMP.UTI_MISSING — every SFT must carry a non-empty UTI.

use super::SftrCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};

/// Check implementation.
pub struct SftrMissingUti;

const CHECK_ID: &str = "SFTR.COMP.UTI_MISSING";

impl SftrCheck for SftrMissingUti {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[SftrRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| r.uti.as_deref().map(str::trim).unwrap_or("").is_empty())
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Sftr,
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

    #[test]
    fn flags_records_without_uti() {
        let records = vec![
            SftrRecord {
                uti: Some("SFTR-OK-001".into()),
                ..Default::default()
            },
            SftrRecord {
                uti: None,
                ..Default::default()
            },
            SftrRecord {
                uti: Some("  ".into()),
                ..Default::default()
            },
        ];
        let ctx = CheckContext::now_with_defaults();
        let issues = SftrMissingUti.run(&records, &ctx);
        assert_eq!(issues.len(), 2);
        assert!(issues.iter().all(|i| i.check_id == "SFTR.COMP.UTI_MISSING"));
    }
}
