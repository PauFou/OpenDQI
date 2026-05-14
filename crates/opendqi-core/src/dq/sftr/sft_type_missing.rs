//! SFTR.COMP.SFT_TYPE_MISSING — the SFT type (REPO / BSB / SLEB /
//! MGLD) is mandatory.

use super::SftrCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};

/// Check implementation.
pub struct SftrSftTypeMissing;

const CHECK_ID: &str = "SFTR.COMP.SFT_TYPE_MISSING";

impl SftrCheck for SftrSftTypeMissing {
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
            .filter(|r| {
                r.sft_type
                    .as_deref()
                    .map(|s| s.trim().is_empty())
                    .unwrap_or(true)
            })
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Sftr,
                severity: Severity::High,
                dimension: DqDimension::Completeness,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("sft_type".into()),
                value: None,
                message: "SFT type is missing.".into(),
                source_file: r.source_file.clone(),
                evidence: Vec::new(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flags_missing() {
        let r = SftrRecord {
            sft_type: None,
            ..Default::default()
        };
        assert_eq!(
            SftrSftTypeMissing
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_present() {
        let r = SftrRecord {
            sft_type: Some("REPO".into()),
            ..Default::default()
        };
        assert!(SftrSftTypeMissing
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
