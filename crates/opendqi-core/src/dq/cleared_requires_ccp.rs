//! EMIR.CON.CLEARED_REQUIRES_CCP — when clearing_status indicates a
//! cleared trade, the central counterparty LEI must be present.

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct ClearedRequiresCcp;

const CHECK_ID: &str = "EMIR.CON.CLEARED_REQUIRES_CCP";

impl Check for ClearedRequiresCcp {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| {
                if !is_cleared(r.clearing_status.as_deref()) {
                    return false;
                }
                r.clearing_ccp_lei
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or("")
                    .is_empty()
            })
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Emir,
                severity: Severity::High,
                dimension: DqDimension::Consistency,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("clearing_ccp_lei".into()),
                value: None,
                message: "Clearing status is 'cleared' but no Central Counterparty LEI is present."
                    .into(),
                source_file: r.source_file.clone(),
            })
            .collect()
    }
}

/// Recognises common spellings of a "cleared" indicator.
fn is_cleared(s: Option<&str>) -> bool {
    let s = match s {
        Some(s) => s.trim(),
        None => return false,
    };
    matches!(
        s.to_ascii_uppercase().as_str(),
        "CLRD" | "Y" | "YES" | "TRUE" | "CLEARED" | "T"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_cleared_without_ccp() {
        let records = vec![
            EmirRecord {
                clearing_status: Some("CLRD".into()),
                clearing_ccp_lei: Some("LCHLDNUS00000000AA".into()),
                ..Default::default()
            },
            EmirRecord {
                clearing_status: Some("CLRD".into()),
                clearing_ccp_lei: None,
                ..Default::default()
            },
            EmirRecord {
                clearing_status: Some("NCLR".into()),
                clearing_ccp_lei: None,
                ..Default::default()
            },
        ];
        let ctx = CheckContext::now_with_defaults();
        let issues = ClearedRequiresCcp.run(&records, &ctx);
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn recognises_alternate_cleared_spellings() {
        assert!(is_cleared(Some("CLRD")));
        assert!(is_cleared(Some("cleared")));
        assert!(is_cleared(Some("Y")));
        assert!(is_cleared(Some("true")));
        assert!(!is_cleared(Some("NCLR")));
        assert!(!is_cleared(Some("")));
        assert!(!is_cleared(None));
    }
}
