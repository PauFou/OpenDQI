//! SFTR.CON.MATURITY_BEFORE_EFFECTIVE — maturity must follow effective date.

use super::SftrCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};

/// Check implementation.
pub struct SftrMaturityBeforeEffective;

const CHECK_ID: &str = "SFTR.CON.MATURITY_BEFORE_EFFECTIVE";

impl SftrCheck for SftrMaturityBeforeEffective {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[SftrRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter_map(|r| {
                let maturity = r.maturity_date?;
                let effective = r.effective_date?;
                if maturity < effective {
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Sftr,
                        severity: Severity::High,
                        dimension: DqDimension::Consistency,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("maturity_date".into()),
                        value: Some(maturity.to_string()),
                        message: format!(
                            "Maturity {maturity} precedes effective date {effective}."
                        ),
                        source_file: r.source_file.clone(),
                    })
                } else {
                    None
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn flags_inverted_dates() {
        let records = vec![
            SftrRecord {
                effective_date: NaiveDate::from_ymd_opt(2026, 4, 1),
                maturity_date: NaiveDate::from_ymd_opt(2026, 7, 1),
                ..Default::default()
            },
            SftrRecord {
                effective_date: NaiveDate::from_ymd_opt(2026, 7, 1),
                maturity_date: NaiveDate::from_ymd_opt(2026, 4, 1),
                ..Default::default()
            },
        ];
        let issues = SftrMaturityBeforeEffective.run(&records, &CheckContext::now_with_defaults());
        assert_eq!(issues.len(), 1);
    }
}
