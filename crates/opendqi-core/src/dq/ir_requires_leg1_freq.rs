//! EMIR.ACC.IR_REQUIRES_LEG1_FREQ — interest-rate trades have at
//! least one leg with a payment frequency.

use super::{Check, CheckContext};
use crate::dq::formats::is_in;
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct IrRequiresLeg1Freq;

const CHECK_ID: &str = "EMIR.ACC.IR_REQUIRES_LEG1_FREQ";
const IR_CODES: &[&str] = &["IR", "RATE"];

impl Check for IrRequiresLeg1Freq {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| {
                r.asset_class
                    .as_deref()
                    .map(|s| is_in(s, IR_CODES))
                    .unwrap_or(false)
                    && r.leg1_payment_frequency
                        .as_deref()
                        .map(str::trim)
                        .unwrap_or("")
                        .is_empty()
            })
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Emir,
                severity: Severity::Warning,
                dimension: DqDimension::Accuracy,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("leg1_payment_frequency".into()),
                value: None,
                message: "Interest-rate trade has no first-leg payment frequency.".into(),
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
    fn flags_ir_without_freq() {
        let r = EmirRecord {
            asset_class: Some("IR".into()),
            leg1_payment_frequency: None,
            ..Default::default()
        };
        assert_eq!(
            IrRequiresLeg1Freq
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_ir_with_freq() {
        let r = EmirRecord {
            asset_class: Some("IR".into()),
            leg1_payment_frequency: Some("3M".into()),
            ..Default::default()
        };
        assert!(IrRequiresLeg1Freq
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
