//! EMIR.ACC.EQ_REQUIRES_UNDERLYING — equity-linked trades must report
//! their underlying instrument.

use super::{Check, CheckContext};
use crate::dq::formats::is_in;
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct EqRequiresUnderlying;

const CHECK_ID: &str = "EMIR.ACC.EQ_REQUIRES_UNDERLYING";
const EQ_CODES: &[&str] = &["EQ", "EQTY"];

impl Check for EqRequiresUnderlying {
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
                    .map(|s| is_in(s, EQ_CODES))
                    .unwrap_or(false)
                    && r.underlying_id
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
                field: Some("underlying_id".into()),
                value: None,
                message: "Equity trade is missing its underlying instrument identifier.".into(),
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
    fn flags_eq_without_underlying() {
        let r = EmirRecord {
            asset_class: Some("EQ".into()),
            underlying_id: None,
            ..Default::default()
        };
        assert_eq!(
            EqRequiresUnderlying
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_eq_with_underlying() {
        let r = EmirRecord {
            asset_class: Some("EQ".into()),
            underlying_id: Some("ID".into()),
            ..Default::default()
        };
        assert!(EqRequiresUnderlying
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
