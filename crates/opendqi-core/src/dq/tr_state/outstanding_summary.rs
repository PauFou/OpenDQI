//! EMIR.TST.OUTSTANDING_SUMMARY — informational: one Info issue per
//! outstanding trade in the TSR. Populates the report's outstanding
//! list without polluting the issue count (Info is excluded from the
//! quality score).

use super::is_outstanding;
use crate::dq::{CheckContext, TrStateCheck};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity, TrStateRecord};

/// Check implementation.
pub struct EmirOutstandingSummary;

const CHECK_ID: &str = "EMIR.TST.OUTSTANDING_SUMMARY";

impl TrStateCheck for EmirOutstandingSummary {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::Info
    }
    fn run(
        &self,
        records: &[TrStateRecord],
        _prior: &[EmirRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        records
            .iter()
            .filter(|r| is_outstanding(r))
            .map(|r| {
                let uti = r.uti.as_deref().unwrap_or("(no UTI)");
                let notional = r
                    .notional_amount
                    .map(|n| format!("{} {}", n, r.notional_currency.as_deref().unwrap_or("?")))
                    .unwrap_or_else(|| "(no notional)".into());
                let maturity = r
                    .maturity_date
                    .map(|d| d.to_string())
                    .unwrap_or_else(|| "(no maturity)".into());
                DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Emir,
                    severity: Severity::Info,
                    dimension: DqDimension::Completeness,
                    record_id: r.record_id.clone(),
                    uti: r.uti.clone(),
                    field: None,
                    value: r.status.clone(),
                    message: format!(
                        "Outstanding trade UTI {uti}: notional={notional}, maturity={maturity}."
                    ),
                    source_file: r.source_file.clone(),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_one_per_outstanding() {
        let recs = vec![
            TrStateRecord {
                uti: Some("U1".into()),
                status: Some("OUTSTANDING".into()),
                ..Default::default()
            },
            TrStateRecord {
                uti: Some("U2".into()),
                status: Some("TERMINATED".into()),
                ..Default::default()
            },
        ];
        assert_eq!(
            EmirOutstandingSummary
                .run(&recs, &[], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }

    #[test]
    fn empty_status_treated_as_outstanding() {
        let recs = vec![TrStateRecord {
            uti: Some("U1".into()),
            status: None,
            ..Default::default()
        }];
        assert_eq!(
            EmirOutstandingSummary
                .run(&recs, &[], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
}
