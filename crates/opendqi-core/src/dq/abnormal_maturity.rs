//! EMIR.ACC.ABNORMAL_MATURITY — maturity dates must look plausible.
//!
//! Three sub-cases raise an issue:
//!
//! 1. Maturity is in the configured `placeholder_dates` list.
//! 2. Maturity is more than `abnormal_maturity_years` after `today`.
//! 3. Maturity precedes `effective_date`.

use chrono::Datelike;

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct AbnormalMaturity;

const CHECK_ID: &str = "EMIR.ACC.ABNORMAL_MATURITY";

impl Check for AbnormalMaturity {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[EmirRecord], ctx: &CheckContext) -> Vec<DqIssue> {
        let mut out = Vec::new();
        let placeholders = &ctx.thresholds.maturity.placeholder_dates;
        let abnormal_years = ctx.thresholds.maturity.abnormal_maturity_years;
        let max_normal = ctx
            .today
            .with_year(ctx.today.year() + abnormal_years)
            .unwrap_or(ctx.today);

        for r in records {
            let Some(maturity) = r.maturity_date else {
                continue;
            };

            let reason = if placeholders.contains(&maturity) {
                Some(format!("Maturity {maturity} is a placeholder date."))
            } else if maturity > max_normal {
                Some(format!(
                    "Maturity {maturity} is more than {abnormal_years} years after {today}.",
                    today = ctx.today
                ))
            } else if let Some(effective) = r.effective_date {
                if maturity < effective {
                    Some(format!(
                        "Maturity {maturity} precedes effective date {effective}."
                    ))
                } else {
                    None
                }
            } else {
                None
            };

            if let Some(message) = reason {
                out.push(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Emir,
                    severity: Severity::Warning,
                    dimension: DqDimension::Accuracy,
                    record_id: r.record_id.clone(),
                    uti: r.uti.clone(),
                    field: Some("maturity_date".into()),
                    value: Some(maturity.to_string()),
                    message,
                    source_file: r.source_file.clone(),
                });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;
    use chrono::NaiveDate;

    fn ctx() -> CheckContext {
        let mut c = CheckContext::now_with_defaults();
        c.today = NaiveDate::from_ymd_opt(2026, 5, 13).unwrap();
        c
    }

    #[test]
    fn placeholder_date_is_flagged() {
        let r = EmirRecord {
            uti: Some("A".into()),
            maturity_date: NaiveDate::from_ymd_opt(2099, 12, 31),
            ..Default::default()
        };
        let issues = AbnormalMaturity.run(&[r], &ctx());
        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("placeholder"));
    }

    #[test]
    fn far_future_is_flagged() {
        let r = EmirRecord {
            uti: Some("B".into()),
            maturity_date: NaiveDate::from_ymd_opt(2200, 6, 1),
            ..Default::default()
        };
        let issues = AbnormalMaturity.run(&[r], &ctx());
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn inverted_dates_is_flagged() {
        let r = EmirRecord {
            uti: Some("C".into()),
            effective_date: NaiveDate::from_ymd_opt(2026, 6, 1),
            maturity_date: NaiveDate::from_ymd_opt(2026, 5, 30),
            ..Default::default()
        };
        let issues = AbnormalMaturity.run(&[r], &ctx());
        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("precedes"));
    }

    #[test]
    fn normal_maturity_passes() {
        let r = EmirRecord {
            uti: Some("D".into()),
            effective_date: NaiveDate::from_ymd_opt(2026, 1, 1),
            maturity_date: NaiveDate::from_ymd_opt(2031, 1, 1),
            ..Default::default()
        };
        let issues = AbnormalMaturity.run(&[r], &ctx());
        assert!(issues.is_empty());
    }

    #[test]
    fn year_addition_does_not_panic() {
        let c = ctx();
        let abnormal_years = c.thresholds.maturity.abnormal_maturity_years;
        let _ = c.today.with_year(c.today.year() + abnormal_years);
    }
}
