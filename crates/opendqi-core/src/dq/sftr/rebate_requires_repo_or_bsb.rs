//! SFTR.CON.REBATE_REQUIRES_REPO_OR_BSB — rebate rate is specific to
//! repo and buy/sell-back transactions.

use super::SftrCheck;
use crate::dq::formats::is_in;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};

/// Check implementation.
pub struct SftrRebateRequiresRepoOrBsb;

const CHECK_ID: &str = "SFTR.CON.REBATE_REQUIRES_REPO_OR_BSB";
const ALLOWED: &[&str] = &["REPO", "BSB"];

impl SftrCheck for SftrRebateRequiresRepoOrBsb {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[SftrRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter_map(|r| {
                r.rebate_rate?;
                let t = r.sft_type.as_deref().unwrap_or("").trim();
                if is_in(t, ALLOWED) {
                    None
                } else {
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Sftr,
                        severity: Severity::Warning,
                        dimension: DqDimension::Consistency,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("rebate_rate".into()),
                        value: r.rebate_rate.map(|x| x.to_string()),
                        message: format!(
                            "Rebate rate is reported but SFT type '{t}' is neither REPO nor BSB."
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
    use rust_decimal::Decimal;
    #[test]
    fn flags_rebate_on_sleb() {
        let r = SftrRecord {
            sft_type: Some("SLEB".into()),
            rebate_rate: Some(Decimal::new(125, 4)),
            ..Default::default()
        };
        assert_eq!(
            SftrRebateRequiresRepoOrBsb
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn accepts_rebate_on_repo() {
        let r = SftrRecord {
            sft_type: Some("REPO".into()),
            rebate_rate: Some(Decimal::new(125, 4)),
            ..Default::default()
        };
        assert!(SftrRebateRequiresRepoOrBsb
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
