//! EMIR.ACC.FX_REQUIRES_LEG2_CURRENCY — FX trades need a second leg
//! currency (a currency pair).

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct FxRequiresLeg2Currency;

const CHECK_ID: &str = "EMIR.ACC.FX_REQUIRES_LEG2_CURRENCY";

impl Check for FxRequiresLeg2Currency {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| {
                r.asset_class
                    .as_deref()
                    .map(|s| s.eq_ignore_ascii_case("FX"))
                    .unwrap_or(false)
                    && r.leg2_notional_currency
                        .as_deref()
                        .map(str::trim)
                        .unwrap_or("")
                        .is_empty()
            })
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Emir,
                severity: Severity::High,
                dimension: DqDimension::Accuracy,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("leg2_notional_currency".into()),
                value: None,
                message: "FX trade is missing the second-leg currency (currency pair).".into(),
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
    fn flags_fx_without_leg2() {
        let r = EmirRecord {
            asset_class: Some("FX".into()),
            leg2_notional_currency: None,
            ..Default::default()
        };
        assert_eq!(
            FxRequiresLeg2Currency
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_fx_with_leg2() {
        let r = EmirRecord {
            asset_class: Some("FX".into()),
            leg2_notional_currency: Some("USD".into()),
            ..Default::default()
        };
        assert!(FxRequiresLeg2Currency
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
