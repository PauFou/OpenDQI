//! `SFTR.T3.IM_RECEIVED_MISSING` — mirror of
//! [`super::im_posted_missing`] on the received side : the
//! portfolio reports `variation_margin_received` OR
//! `excess_collateral_received` but `initial_margin_received`
//! is `None`.

use super::SftrMsrCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrMarginStateRecord};

/// Check implementation.
pub struct SftrT3ImReceivedMissing;

const CHECK_ID: &str = "SFTR.T3.IM_RECEIVED_MISSING";

impl SftrMsrCheck for SftrT3ImReceivedMissing {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[SftrMarginStateRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        let mut out = Vec::new();
        for r in records {
            let received_block_present =
                r.variation_margin_received.is_some() || r.excess_collateral_received.is_some();
            if received_block_present && r.initial_margin_received.is_none() {
                out.push(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Sftr,
                    severity: Severity::High,
                    dimension: DqDimension::Completeness,
                    record_id: r.record_id.clone(),
                    uti: r.collateral_portfolio_code.clone(),
                    field: Some("initial_margin_received".into()),
                    value: None,
                    message:
                        "initial_margin_received is missing while other received-side amounts \
                              are reported"
                            .into(),
                    source_file: r.source_file.clone(),
                    evidence: Vec::new(),
                });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use super::*;

    fn ctx() -> CheckContext {
        CheckContext {
            thresholds: Default::default(),
            today: chrono::NaiveDate::from_ymd_opt(2026, 5, 13).unwrap(),
            now: chrono::DateTime::parse_from_rfc3339("2026-05-13T08:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
        }
    }

    #[test]
    fn fires_when_only_vm_received_set() {
        let r = SftrMarginStateRecord {
            collateral_portfolio_code: Some("P1".into()),
            variation_margin_received: Some(Decimal::from(50)),
            ..Default::default()
        };
        let out = SftrT3ImReceivedMissing.run(&[r], &ctx());
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn does_not_fire_when_im_received_present() {
        let r = SftrMarginStateRecord {
            collateral_portfolio_code: Some("P1".into()),
            initial_margin_received: Some(Decimal::from(100)),
            variation_margin_received: Some(Decimal::from(50)),
            ..Default::default()
        };
        let out = SftrT3ImReceivedMissing.run(&[r], &ctx());
        assert!(out.is_empty());
    }
}
