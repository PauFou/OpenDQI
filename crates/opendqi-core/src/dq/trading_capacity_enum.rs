//! EMIR.VLD.TRADING_CAPACITY_ENUM — trading capacity must be AGEN or PRIN.

use super::{Check, CheckContext};
use crate::dq::formats::is_in;
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct TradingCapacityEnum;

const CHECK_ID: &str = "EMIR.VLD.TRADING_CAPACITY_ENUM";
const ALLOWED: &[&str] = &["AGEN", "PRIN"];

impl Check for TradingCapacityEnum {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Validity
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter_map(|r| {
                let v = r.trading_capacity.as_deref()?.trim();
                if v.is_empty() || is_in(v, ALLOWED) {
                    None
                } else {
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Emir,
                        severity: Severity::Warning,
                        dimension: DqDimension::Validity,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("trading_capacity".into()),
                        value: Some(v.to_owned()),
                        message: format!(
                            "Trading capacity '{v}' is not in the allowed set {{AGEN, PRIN}}."
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
    fn flags_unknown() {
        let r = EmirRecord {
            trading_capacity: Some("XXX".into()),
            ..Default::default()
        };
        assert_eq!(
            TradingCapacityEnum
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn accepts_prin() {
        let r = EmirRecord {
            trading_capacity: Some("PRIN".into()),
            ..Default::default()
        };
        assert!(TradingCapacityEnum
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
