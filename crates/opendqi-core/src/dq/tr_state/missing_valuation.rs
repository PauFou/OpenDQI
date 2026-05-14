//! EMIR.TST.MISSING_VALUATION — outstanding trade in the TSR with
//! no valuation amount.

use super::is_outstanding;
use crate::dq::{CheckContext, TrStateCheck};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity, TrStateRecord};

/// Check implementation.
pub struct EmirMissingValuationOnTsr;

const CHECK_ID: &str = "EMIR.TST.MISSING_VALUATION";

impl TrStateCheck for EmirMissingValuationOnTsr {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(
        &self,
        records: &[TrStateRecord],
        _prior: &[EmirRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| is_outstanding(r) && r.valuation_amount.is_none())
            .map(|r| DqIssue {
                check_id: CHECK_ID.into(),
                regime: Regime::Emir,
                severity: Severity::High,
                dimension: DqDimension::Completeness,
                record_id: r.record_id.clone(),
                uti: r.uti.clone(),
                field: Some("valuation_amount".into()),
                value: None,
                message: "TR shows this trade as outstanding but holds no valuation amount.".into(),
                source_file: r.source_file.clone(),
                evidence: Vec::new(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    #[test]
    fn flags_missing_valuation() {
        let rec = TrStateRecord {
            uti: Some("U1".into()),
            status: Some("OUTSTANDING".into()),
            valuation_amount: None,
            ..Default::default()
        };
        assert_eq!(
            EmirMissingValuationOnTsr
                .run(&[rec], &[], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }

    #[test]
    fn ignores_when_valuation_present() {
        let rec = TrStateRecord {
            uti: Some("U1".into()),
            status: Some("OUTSTANDING".into()),
            valuation_amount: Some(Decimal::from(1000)),
            ..Default::default()
        };
        assert!(EmirMissingValuationOnTsr
            .run(&[rec], &[], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
