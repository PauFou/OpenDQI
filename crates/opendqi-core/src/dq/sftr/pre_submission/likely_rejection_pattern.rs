//! SFTR.PSC.LIKELY_REJECTION_PATTERN — SFTR mirror of the EMIR PSC
//! pattern check. Maps `top_causes[].suggested_check` IDs (canonical
//! SFTR check IDs) to a built-in predicate on `SftrRecord`; when a
//! record fails the predicate we fire a Warning citing the cause's
//! historical rank and share.
//!
//! Predicate table is intentionally narrow (v1): UTI, loan/collateral
//! value, SFT type, counterparty LEI completeness, plus negative loan
//! / collateral and haircut-out-of-range accuracy. Extend in this
//! file alongside the analytics export's
//! `suggested_check_for_reason`.

use super::SftrPreSubmissionCheck;
use crate::dq::CheckContext;
use crate::model::{
    DqDimension, DqIssue, EvidenceItem, Regime, RejectionProfile, Severity, SftrRecord,
};

/// Check implementation.
pub struct SftrPscLikelyRejectionPattern;

const CHECK_ID: &str = "SFTR.PSC.LIKELY_REJECTION_PATTERN";

type Predicate = fn(&SftrRecord) -> bool;

fn predicate_for(check_id: &str) -> Option<Predicate> {
    match check_id {
        "SFTR.COMP.UTI_MISSING" => Some(uti_missing),
        "SFTR.COMP.COLLATERAL_VALUE_MISSING" => Some(collateral_value_missing),
        "SFTR.COMP.SFT_TYPE_MISSING" => Some(sft_type_missing),
        "SFTR.COMP.COUNTERPARTY_1_MISSING" => Some(cp1_missing),
        "SFTR.COMP.COUNTERPARTY_2_MISSING" => Some(cp2_missing),
        "SFTR.ACC.NEGATIVE_LOAN" => Some(negative_loan),
        "SFTR.ACC.NEGATIVE_COLLATERAL" => Some(negative_collateral),
        "SFTR.ACC.HAIRCUT_OUT_OF_RANGE" => Some(haircut_out_of_range),
        _ => None,
    }
}

fn uti_missing(r: &SftrRecord) -> bool {
    r.uti.as_deref().map(str::trim).unwrap_or("").is_empty()
}
fn collateral_value_missing(r: &SftrRecord) -> bool {
    r.collateral_value.is_none()
}
fn sft_type_missing(r: &SftrRecord) -> bool {
    r.sft_type
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
}
fn cp1_missing(r: &SftrRecord) -> bool {
    r.counterparty_1
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
}
fn cp2_missing(r: &SftrRecord) -> bool {
    r.counterparty_2
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
}
fn negative_loan(r: &SftrRecord) -> bool {
    r.loan_value.map(|v| v.is_sign_negative()).unwrap_or(false)
}
fn negative_collateral(r: &SftrRecord) -> bool {
    r.collateral_value
        .map(|v| v.is_sign_negative())
        .unwrap_or(false)
}
fn haircut_out_of_range(r: &SftrRecord) -> bool {
    use rust_decimal::Decimal;
    match r.haircut {
        Some(h) => h < Decimal::ZERO || h > Decimal::ONE,
        None => false,
    }
}

impl SftrPreSubmissionCheck for SftrPscLikelyRejectionPattern {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Validity
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(
        &self,
        records: &[SftrRecord],
        profile: &RejectionProfile,
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        if profile.top_causes.is_empty() || profile.total_feedbacks == 0 {
            return Vec::new();
        }
        let mut out = Vec::new();
        for cause in &profile.top_causes {
            let suggested = match cause.suggested_check.as_deref() {
                Some(s) => s,
                None => continue,
            };
            let predicate = match predicate_for(suggested) {
                Some(p) => p,
                None => continue,
            };
            let share_pct = (cause.count as f64 / profile.total_feedbacks.max(1) as f64) * 100.0;
            for r in records.iter().filter(|r| predicate(r)) {
                out.push(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Sftr,
                    severity: Severity::Warning,
                    dimension: DqDimension::Validity,
                    record_id: r.record_id.clone(),
                    uti: r.uti.clone(),
                    field: None,
                    value: Some(cause.reason_code.clone()),
                    message: format!(
                        "SFT record matches {suggested} — historically the TR's #{rank} rejection cause ({cnt}/{total}, {share:.1}%, reason_code={code}).",
                        rank = profile
                            .top_causes
                            .iter()
                            .position(|c| c.reason_code == cause.reason_code)
                            .map(|i| i + 1)
                            .unwrap_or(0),
                        cnt = cause.count,
                        total = profile.total_feedbacks,
                        share = share_pct,
                        code = cause.reason_code,
                    ),
                    source_file: r.source_file.clone(),
                    evidence: vec![EvidenceItem {
                        field: "suggested_check".into(),
                        before: None,
                        after: Some(suggested.to_owned()),
                        source_line: None,
                    }],
                });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{RejectionCause, RejectionProfile};

    fn profile(total: u64, causes: &[(&str, u64, &str)]) -> RejectionProfile {
        RejectionProfile {
            total_feedbacks: total,
            top_causes: causes
                .iter()
                .map(|(code, n, sc)| RejectionCause {
                    reason_code: (*code).into(),
                    count: *n,
                    suggested_check: Some((*sc).into()),
                })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn flags_records_missing_collateral_value_when_cause_is_top() {
        let p = profile(
            100,
            &[("SFTRVAL02", 50, "SFTR.COMP.COLLATERAL_VALUE_MISSING")],
        );
        let records = vec![
            SftrRecord {
                record_id: Some("ok".into()),
                collateral_value: Some(rust_decimal::Decimal::from(1000)),
                ..Default::default()
            },
            SftrRecord {
                record_id: Some("bad".into()),
                collateral_value: None,
                ..Default::default()
            },
        ];
        let ctx = CheckContext::now_with_defaults();
        let issues = SftrPscLikelyRejectionPattern.run(&records, &p, &ctx);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].record_id.as_deref(), Some("bad"));
        assert!(issues[0].message.contains("50.0%"));
    }

    #[test]
    fn skips_causes_without_predicate_mapping() {
        let p = profile(100, &[("XYZ", 50, "SFTR.UNKNOWN.CHECK")]);
        let records = vec![SftrRecord {
            uti: None,
            ..Default::default()
        }];
        let ctx = CheckContext::now_with_defaults();
        assert!(SftrPscLikelyRejectionPattern
            .run(&records, &p, &ctx)
            .is_empty());
    }

    #[test]
    fn empty_profile_yields_nothing() {
        let p = RejectionProfile::default();
        let records = vec![SftrRecord {
            uti: None,
            ..Default::default()
        }];
        let ctx = CheckContext::now_with_defaults();
        assert!(SftrPscLikelyRejectionPattern
            .run(&records, &p, &ctx)
            .is_empty());
    }
}
