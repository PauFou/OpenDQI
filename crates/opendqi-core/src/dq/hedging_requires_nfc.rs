//! EMIR.CON.HEDGING_REQUIRES_NFC — only Non-Financial counterparties
//! report the hedging indicator under EMIR.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct HedgingRequiresNfc;

const CHECK_ID: &str = "EMIR.CON.HEDGING_REQUIRES_NFC";

impl Check for HedgingRequiresNfc {
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
            .filter(|r| {
                r.hedging_indicator == Some(true)
                    && r.nature.as_deref().map(|s| !s.eq_ignore_ascii_case("N")).unwrap_or(true)
            })
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Emir,
                severity: Severity::Warning,
                dimension: DqDimension::Consistency,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("hedging_indicator".into()),
                value: Some("true".into()),
                message: "Hedging indicator is true but the reporting counterparty is not declared as Non-Financial (nature != 'N').".into(),
                source_file: r.source_file.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flags_hedging_on_fc() {
        let r = EmirRecord {
            hedging_indicator: Some(true),
            nature: Some("F".into()),
            ..Default::default()
        };
        assert_eq!(
            HedgingRequiresNfc
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn ignores_hedging_on_nfc() {
        let r = EmirRecord {
            hedging_indicator: Some(true),
            nature: Some("N".into()),
            ..Default::default()
        };
        assert!(HedgingRequiresNfc
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
    #[test]
    fn ignores_when_no_hedging() {
        let r = EmirRecord {
            hedging_indicator: Some(false),
            nature: Some("F".into()),
            ..Default::default()
        };
        assert!(HedgingRequiresNfc
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
