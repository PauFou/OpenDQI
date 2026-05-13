//! EMIR.ACC.CR_REQUIRES_UNDERLYING — credit derivatives must report
//! the reference entity / underlying instrument.

use super::{Check, CheckContext};
use crate::dq::formats::is_in;
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct CrRequiresUnderlying;

const CHECK_ID: &str = "EMIR.ACC.CR_REQUIRES_UNDERLYING";
const CR_CODES: &[&str] = &["CR", "CRDT"];

impl Check for CrRequiresUnderlying {
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
                    .map(|s| is_in(s, CR_CODES))
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
                message: "Credit derivative is missing the reference-entity identifier.".into(),
                source_file: r.source_file.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flags_cr_without_underlying() {
        let r = EmirRecord {
            asset_class: Some("CR".into()),
            underlying_id: None,
            ..Default::default()
        };
        assert_eq!(
            CrRequiresUnderlying
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_cr_with_underlying() {
        let r = EmirRecord {
            asset_class: Some("CR".into()),
            underlying_id: Some("ID".into()),
            ..Default::default()
        };
        assert!(CrRequiresUnderlying
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
