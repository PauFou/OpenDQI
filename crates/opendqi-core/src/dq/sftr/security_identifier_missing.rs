//! SFTR.COMP.SECURITY_ID_MISSING — the principal SFT leg of securities
//! lending / borrowing (`SLEB`) and buy/sell-back (`BSB`) trades must
//! carry a `security_identifier` (ISIN of the security being lent or
//! bought back). Margin lending (`MGLD`) is a cash-margin product so
//! the field is not required; repos (`REPO`) often reference a basket
//! and the per-record ISIN is optional, so we only flag the two
//! categories where the security ISIN is unambiguously required.

use super::SftrCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};

/// Check implementation.
pub struct SftrSecurityIdentifierMissing;

const CHECK_ID: &str = "SFTR.COMP.SECURITY_ID_MISSING";

fn requires_security_id(sft_type: &str) -> bool {
    matches!(
        sft_type.trim().to_ascii_uppercase().as_str(),
        "SLEB" | "BSB"
    )
}

impl SftrCheck for SftrSecurityIdentifierMissing {
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
            .filter(|r| match r.sft_type.as_deref() {
                Some(t) => requires_security_id(t),
                None => false,
            })
            .filter(|r| {
                r.security_identifier
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or("")
                    .is_empty()
            })
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Sftr,
                severity: Severity::High,
                dimension: DqDimension::Completeness,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("security_identifier".into()),
                value: r.sft_type.clone(),
                message: format!(
                    "SFT type {} requires the security identifier (ISIN) of the principal leg.",
                    r.sft_type.as_deref().unwrap_or("?")
                ),
                source_file: r.source_file.clone(),
                evidence: Vec::new(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(sft_type: &str, sec_id: Option<&str>) -> SftrRecord {
        SftrRecord {
            uti: Some(format!("U-{sft_type}")),
            sft_type: Some(sft_type.into()),
            security_identifier: sec_id.map(str::to_owned),
            ..Default::default()
        }
    }

    #[test]
    fn flags_sleb_and_bsb_without_security_id() {
        let records = vec![
            rec("SLEB", None),
            rec("BSB", Some("  ")),
            rec("SLEB", Some("FR0010000001")),
            rec("MGLD", None),
            rec("REPO", None),
        ];
        let ctx = CheckContext::now_with_defaults();
        let issues = SftrSecurityIdentifierMissing.run(&records, &ctx);
        assert_eq!(issues.len(), 2);
        assert!(issues
            .iter()
            .all(|i| i.check_id == "SFTR.COMP.SECURITY_ID_MISSING"));
        assert!(issues.iter().any(|i| i.uti.as_deref() == Some("U-SLEB")));
        assert!(issues.iter().any(|i| i.uti.as_deref() == Some("U-BSB")));
    }

    #[test]
    fn ignores_mgld_and_repo_when_security_id_missing() {
        let records = vec![rec("MGLD", None), rec("REPO", None)];
        let ctx = CheckContext::now_with_defaults();
        let issues = SftrSecurityIdentifierMissing.run(&records, &ctx);
        assert!(issues.is_empty());
    }

    #[test]
    fn ignores_records_without_sft_type() {
        let records = vec![SftrRecord {
            sft_type: None,
            security_identifier: None,
            ..Default::default()
        }];
        let ctx = CheckContext::now_with_defaults();
        let issues = SftrSecurityIdentifierMissing.run(&records, &ctx);
        assert!(issues.is_empty());
    }
}
