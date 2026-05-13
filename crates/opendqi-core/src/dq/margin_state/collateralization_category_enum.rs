//! EMIR.MSR.COLLATERALIZATION_CATEGORY_ENUM — `collateralization_category`
//! ∈ {FCOL, PCOL, UCOL, OCOL}.

use super::MarginStateCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, EmirRecord, MarginStateRecord, Regime, Severity};

/// Check implementation.
pub struct EmirMsrCollateralizationCategoryEnum;

const CHECK_ID: &str = "EMIR.MSR.COLLATERALIZATION_CATEGORY_ENUM";
const ALLOWED: &[&str] = &["FCOL", "PCOL", "UCOL", "OCOL"];

impl MarginStateCheck for EmirMsrCollateralizationCategoryEnum {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Validity
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(
        &self,
        records: &[MarginStateRecord],
        _prior: &[EmirRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        let mut out = Vec::new();
        for r in records {
            if let Some(c) = r.collateralization_category.as_deref() {
                let upper = c.trim().to_uppercase();
                if !upper.is_empty() && !ALLOWED.contains(&upper.as_str()) {
                    out.push(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Emir,
                        severity: Severity::High,
                        dimension: DqDimension::Validity,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("collateralization_category".into()),
                        value: Some(c.to_owned()),
                        message: format!(
                            "Collateralisation category '{c}' is not in {{FCOL, PCOL, UCOL, OCOL}}."
                        ),
                        source_file: r.source_file.clone(),
                    });
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> CheckContext {
        CheckContext {
            thresholds: Default::default(),
            today: chrono::NaiveDate::from_ymd_opt(2026, 5, 13).unwrap(),
            now: chrono::DateTime::parse_from_rfc3339("2026-05-13T08:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
        }
    }

    #[test]
    fn flags_invalid() {
        let r = MarginStateRecord {
            collateralization_category: Some("XYZ".into()),
            ..Default::default()
        };
        let out = EmirMsrCollateralizationCategoryEnum.run(&[r], &[], &ctx());
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn accepts_fcol() {
        let r = MarginStateRecord {
            collateralization_category: Some("FCOL".into()),
            ..Default::default()
        };
        let out = EmirMsrCollateralizationCategoryEnum.run(&[r], &[], &ctx());
        assert!(out.is_empty());
    }
}
