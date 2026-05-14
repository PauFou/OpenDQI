//! EMIR.CON.NCLR_FORBIDS_CCP — a non-cleared trade must not declare
//! a CCP LEI.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct NclrForbidsCcp;

const CHECK_ID: &str = "EMIR.CON.NCLR_FORBIDS_CCP";

impl Check for NclrForbidsCcp {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| {
                let is_nclr = r
                    .clearing_status
                    .as_deref()
                    .map(|s| s.eq_ignore_ascii_case("NCLR"))
                    .unwrap_or(false);
                is_nclr
                    && !r
                        .clearing_ccp_lei
                        .as_deref()
                        .map(str::trim)
                        .unwrap_or("")
                        .is_empty()
            })
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Emir,
                severity: Severity::Warning,
                dimension: DqDimension::Consistency,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("clearing_ccp_lei".into()),
                value: r.clearing_ccp_lei.clone(),
                message: "Clearing status is NCLR (not cleared) but a CCP LEI is also reported."
                    .into(),
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
    fn flags_nclr_with_ccp() {
        let r = EmirRecord {
            clearing_status: Some("NCLR".into()),
            clearing_ccp_lei: Some("ABCDEFGHIJKLMNOPQR01".into()),
            ..Default::default()
        };
        assert_eq!(
            NclrForbidsCcp
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_clrd_with_ccp() {
        let r = EmirRecord {
            clearing_status: Some("CLRD".into()),
            clearing_ccp_lei: Some("ABCDEFGHIJKLMNOPQR01".into()),
            ..Default::default()
        };
        assert!(NclrForbidsCcp
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
