//! EMIR.TST.PLACEHOLDER_MATURITY — TSR maturity date matches one of
//! the configured placeholder dates (e.g. 1900-01-01, 9999-12-31,
//! 2099-12-31).

use crate::dq::{CheckContext, TrStateCheck};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity, TrStateRecord};

/// Check implementation.
pub struct EmirPlaceholderMaturity;

const CHECK_ID: &str = "EMIR.TST.PLACEHOLDER_MATURITY";

impl TrStateCheck for EmirPlaceholderMaturity {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(
        &self,
        records: &[TrStateRecord],
        _prior: &[EmirRecord],
        ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        records
            .iter()
            .filter_map(|r| {
                let maturity = r.maturity_date?;
                let is_placeholder = ctx
                    .thresholds
                    .maturity
                    .placeholder_dates
                    .contains(&maturity);
                if !is_placeholder {
                    return None;
                }
                let s = maturity.to_string();
                Some(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Emir,
                    severity: Severity::Warning,
                    dimension: DqDimension::Accuracy,
                    record_id: r.record_id.clone(),
                    uti: r.uti.clone(),
                    field: Some("maturity_date".into()),
                    value: Some(s.clone()),
                    message: format!("TR holds a placeholder maturity date ({s}) on this trade."),
                    source_file: r.source_file.clone(),
                    evidence: Vec::new(),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn flags_placeholder_date() {
        let rec = TrStateRecord {
            uti: Some("U1".into()),
            maturity_date: NaiveDate::from_ymd_opt(9999, 12, 31),
            ..Default::default()
        };
        assert_eq!(
            EmirPlaceholderMaturity
                .run(&[rec], &[], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }

    #[test]
    fn ignores_regular_date() {
        let rec = TrStateRecord {
            uti: Some("U1".into()),
            maturity_date: NaiveDate::from_ymd_opt(2030, 6, 30),
            ..Default::default()
        };
        assert!(EmirPlaceholderMaturity
            .run(&[rec], &[], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
