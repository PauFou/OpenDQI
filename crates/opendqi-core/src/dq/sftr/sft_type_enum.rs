//! SFTR.VLD.SFT_TYPE_ENUM — SFT type must be one of REPO / BSB /
//! SLEB / MGLD.

use super::SftrCheck;
use crate::dq::formats::is_in;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};

/// Check implementation.
pub struct SftrSftTypeEnum;

const CHECK_ID: &str = "SFTR.VLD.SFT_TYPE_ENUM";
const ALLOWED: &[&str] = &["REPO", "BSB", "SLEB", "MGLD"];

impl SftrCheck for SftrSftTypeEnum {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Validity
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[SftrRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter_map(|r| {
                let t = r.sft_type.as_deref()?.trim();
                if t.is_empty() || is_in(t, ALLOWED) {
                    None
                } else {
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Sftr,
                        severity: Severity::High,
                        dimension: DqDimension::Validity,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("sft_type".into()),
                        value: Some(t.to_owned()),
                        message: format!("SFT type '{t}' is not one of REPO / BSB / SLEB / MGLD."),
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
    fn flags_unknown() {
        let r = SftrRecord {
            sft_type: Some("FOOO".into()),
            ..Default::default()
        };
        assert_eq!(
            SftrSftTypeEnum
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn accepts_repo() {
        let r = SftrRecord {
            sft_type: Some("REPO".into()),
            ..Default::default()
        };
        assert!(SftrSftTypeEnum
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
