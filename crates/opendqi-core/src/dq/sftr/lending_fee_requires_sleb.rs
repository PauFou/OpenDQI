//! SFTR.CON.LENDING_FEE_REQUIRES_SLEB — lending fee is specific to
//! securities lending transactions.

use super::SftrCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};

/// Check implementation.
pub struct SftrLendingFeeRequiresSleb;

const CHECK_ID: &str = "SFTR.CON.LENDING_FEE_REQUIRES_SLEB";

impl SftrCheck for SftrLendingFeeRequiresSleb {
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
                r.lending_fee?;
                let t = r.sft_type.as_deref().unwrap_or("").trim();
                if t.eq_ignore_ascii_case("SLEB") {
                    None
                } else {
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Sftr,
                        severity: Severity::Warning,
                        dimension: DqDimension::Consistency,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("lending_fee".into()),
                        value: r.lending_fee.map(|x| x.to_string()),
                        message: format!("Lending fee is reported but SFT type '{t}' is not SLEB."),
                        source_file: r.source_file.clone(),
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
    fn flags_fee_on_repo() {
        let r = SftrRecord {
            sft_type: Some("REPO".into()),
            lending_fee: Some(Decimal::new(50, 4)),
            ..Default::default()
        };
        assert_eq!(
            SftrLendingFeeRequiresSleb
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn accepts_fee_on_sleb() {
        let r = SftrRecord {
            sft_type: Some("SLEB".into()),
            lending_fee: Some(Decimal::new(50, 4)),
            ..Default::default()
        };
        assert!(SftrLendingFeeRequiresSleb
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
