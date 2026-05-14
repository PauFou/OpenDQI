//! EMIR.VLD.LEI_FORMAT_OC — other counterparty LEI must match
//! ISO 17442 (EMIR-VR-1006-01).

use super::{Check, CheckContext};
use crate::dq::formats::is_valid_lei;
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct LeiFormatOc;

const CHECK_ID: &str = "EMIR.VLD.LEI_FORMAT_OC";

impl Check for LeiFormatOc {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Validity
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter_map(|r| {
                let lei = r.counterparty_2.as_deref()?.trim();
                if lei.is_empty() || is_valid_lei(lei) {
                    None
                } else {
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Emir,
                        severity: Severity::High,
                        dimension: DqDimension::Validity,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("counterparty_2".into()),
                        value: Some(lei.to_owned()),
                        message: format!(
                            "Other counterparty LEI '{lei}' is not a valid ISO 17442 identifier (EMIR-VR-1006-01)."
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
    fn flags_invalid_lei_format_on_oc() {
        let records = vec![
            EmirRecord {
                counterparty_2: Some("ABCDEFGHIJKLMNOPQR01".into()),
                ..Default::default()
            },
            EmirRecord {
                counterparty_2: Some("ABC".into()),
                ..Default::default()
            },
        ];
        let ctx = CheckContext::now_with_defaults();
        let issues = LeiFormatOc.run(&records, &ctx);
        assert_eq!(issues.len(), 1);
    }
}
