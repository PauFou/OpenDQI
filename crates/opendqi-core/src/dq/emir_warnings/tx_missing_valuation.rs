//! EMIR.WRN.TX_MISSING_VALUATION — the TR explicitly listed this
//! transaction (in the auth.106 `Wrnngs/TxDtls` breakdown) as having
//! no valuation reported. One issue per flagged `TxDtls`.

use super::{build_issue_tx, tx_counterparty_label, WarningsTransactionCheck};
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Severity, WarningsTransactionRecord};

/// Check implementation.
pub struct EmirWrnTxMissingValuation;

const CHECK_ID: &str = "EMIR.WRN.TX_MISSING_VALUATION";

impl WarningsTransactionCheck for EmirWrnTxMissingValuation {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[WarningsTransactionRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| r.warning_category.as_deref() == Some("MissingValuation"))
            .map(|r| {
                let mut issue =
                    build_issue_tx(CHECK_ID, Severity::High, DqDimension::Completeness, r);
                issue.message = format!(
                    "Trade Repository flagged SFT {} ({}) for missing valuation.",
                    r.uti.as_deref().unwrap_or("(no UTI)"),
                    tx_counterparty_label(r),
                );
                issue
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fires_only_on_missing_valuation_category() {
        let recs = vec![
            WarningsTransactionRecord {
                uti: Some("U1".into()),
                warning_category: Some("MissingValuation".into()),
                ..Default::default()
            },
            WarningsTransactionRecord {
                uti: Some("U2".into()),
                warning_category: Some("AbnormalValue".into()),
                ..Default::default()
            },
        ];
        let ctx = CheckContext::now_with_defaults();
        let issues = EmirWrnTxMissingValuation.run(&recs, &ctx);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].check_id, CHECK_ID);
        assert_eq!(issues[0].uti.as_deref(), Some("U1"));
    }
}
