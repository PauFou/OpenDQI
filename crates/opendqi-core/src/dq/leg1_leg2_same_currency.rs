//! EMIR.CON.LEG1_LEG2_SAME_CURRENCY — when both legs of a swap are
//! reported, having identical currencies is unusual for cross-currency
//! products (legitimate for single-currency IRS).

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct Leg1Leg2SameCurrency;

const CHECK_ID: &str = "EMIR.CON.LEG1_LEG2_SAME_CURRENCY";

impl Check for Leg1Leg2SameCurrency {
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
            .filter_map(|r| {
                let a = r.notional_currency.as_deref()?.trim();
                let b = r.leg2_notional_currency.as_deref()?.trim();
                if a.is_empty() || b.is_empty() || !a.eq_ignore_ascii_case(b) {
                    None
                } else {
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Emir,
                        severity: Severity::Warning,
                        dimension: DqDimension::Consistency,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("leg2_notional_currency".into()),
                        value: Some(b.to_owned()),
                        message: format!(
                            "Both legs reported in '{a}'. Expected for single-currency IRS but unusual for cross-currency products."
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
    fn flags_same_currency() {
        let r = EmirRecord {
            notional_currency: Some("EUR".into()),
            leg2_notional_currency: Some("EUR".into()),
            ..Default::default()
        };
        assert_eq!(
            Leg1Leg2SameCurrency
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_different() {
        let r = EmirRecord {
            notional_currency: Some("EUR".into()),
            leg2_notional_currency: Some("USD".into()),
            ..Default::default()
        };
        assert!(Leg1Leg2SameCurrency
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
