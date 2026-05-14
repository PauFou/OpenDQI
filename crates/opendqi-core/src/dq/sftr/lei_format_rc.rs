//! SFTR.VLD.LEI_FORMAT_RC — reporting counterparty LEI shape (ISO 17442).

use super::SftrCheck;
use crate::dq::formats::is_valid_lei;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};

/// Check implementation.
pub struct SftrLeiFormatRc;

const CHECK_ID: &str = "SFTR.VLD.LEI_FORMAT_RC";

impl SftrCheck for SftrLeiFormatRc {
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
                let lei = r.counterparty_1.as_deref()?.trim();
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
                        field: Some("counterparty_1".into()),
                        value: Some(lei.to_owned()),
                        message: format!(
                            "Reporting counterparty LEI '{lei}' is not a valid ISO 17442 identifier."
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
    fn flags_invalid_rc_lei() {
        let records = vec![
            SftrRecord {
                counterparty_1: Some("ABCDEFGHIJKLMNOPQR01".into()),
                ..Default::default()
            },
            SftrRecord {
                counterparty_1: Some("FAKE_LEI".into()),
                ..Default::default()
            },
        ];
        let ctx = CheckContext::now_with_defaults();
        let issues = SftrLeiFormatRc.run(&records, &ctx);
        assert_eq!(issues.len(), 1);
    }
}
