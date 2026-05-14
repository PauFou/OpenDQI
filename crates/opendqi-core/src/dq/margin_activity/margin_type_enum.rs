//! EMIR.MAR.MARGIN_TYPE_ENUM — action_type ∈ {MARU, MARV, MARC, MARN}.

use super::MarginActivityCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, MarginActivityRecord, Regime, Severity};

/// Check implementation.
pub struct EmirMarMarginTypeEnum;

const CHECK_ID: &str = "EMIR.MAR.MARGIN_TYPE_ENUM";
const ALLOWED: &[&str] = &["MARU", "MARV", "MARC", "MARN"];

impl MarginActivityCheck for EmirMarMarginTypeEnum {
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
        records: &[MarginActivityRecord],
        _prior: &[MarginActivityRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        let mut out = Vec::new();
        for r in records {
            if let Some(a) = r.action_type.as_deref() {
                let upper = a.trim().to_uppercase();
                if !upper.is_empty() && !ALLOWED.contains(&upper.as_str()) {
                    out.push(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Emir,
                        severity: Severity::High,
                        dimension: DqDimension::Validity,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("action_type".into()),
                        value: Some(a.to_owned()),
                        message: format!(
                            "Action type '{a}' is not in the EMIR margin set {{MARU, MARV, MARC, MARN}}."
                        ),
                        source_file: r.source_file.clone(),
                        evidence: Vec::new(),
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
    fn flags_invalid_action_type() {
        let r = MarginActivityRecord {
            action_type: Some("XYZ".into()),
            ..Default::default()
        };
        let out = EmirMarMarginTypeEnum.run(&[r], &[], &ctx());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].check_id, CHECK_ID);
    }

    #[test]
    fn accepts_marv() {
        let r = MarginActivityRecord {
            action_type: Some("MARV".into()),
            ..Default::default()
        };
        let out = EmirMarMarginTypeEnum.run(&[r], &[], &ctx());
        assert!(out.is_empty());
    }
}
