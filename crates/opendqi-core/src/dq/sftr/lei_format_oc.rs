//! SFTR.VLD.LEI_FORMAT_OC — other counterparty LEI shape (ISO 17442).

use super::SftrCheck;
use crate::dq::formats::is_valid_lei;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};

/// Check implementation.
pub struct SftrLeiFormatOc;

const CHECK_ID: &str = "SFTR.VLD.LEI_FORMAT_OC";

impl SftrCheck for SftrLeiFormatOc {
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
                let lei = r.counterparty_2.as_deref()?.trim();
                if lei.is_empty() || is_valid_lei(lei) {
                    None
                } else {
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Sftr,
                        severity: Severity::High,
                        dimension: DqDimension::Validity,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("counterparty_2".into()),
                        value: Some(lei.to_owned()),
                        message: format!(
                            "Other counterparty LEI '{lei}' is not a valid ISO 17442 identifier."
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
    fn flags_invalid_oc_lei() {
        let records = vec![
            SftrRecord {
                counterparty_2: Some("ABCDEFGHIJKLMNOPQR01".into()),
                ..Default::default()
            },
            SftrRecord {
                counterparty_2: Some("ABC".into()),
                ..Default::default()
            },
        ];
        let ctx = CheckContext::now_with_defaults();
        let issues = SftrLeiFormatOc.run(&records, &ctx);
        assert_eq!(issues.len(), 1);
    }
}
