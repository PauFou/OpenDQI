//! EMIR.PSC.LIKELY_REJECTION_PATTERN — flag any record matching a
//! data-quality predicate that the TR has rejected most often in the
//! analytics window. The profile's `top_causes[].suggested_check`
//! field carries a canonical OpenDQI check ID (e.g.
//! `EMIR.COMP.UTI_MISSING`); this PSC check applies a tiny built-in
//! mapping from check ID to a predicate on `EmirRecord`. When a
//! record fails the predicate, we emit a Warning citing the
//! historical cause and its frequency.
//!
//! The mapping is intentionally narrow (v1): it covers the most
//! common completeness/validity issues seen in real TR feedback. Add
//! new entries when extending the family.

use super::PreSubmissionCheck;
use crate::dq::CheckContext;
use crate::model::{
    DqDimension, DqIssue, EmirRecord, EvidenceItem, Regime, RejectionProfile, Severity,
};

/// Check implementation.
pub struct EmirPscLikelyRejectionPattern;

const CHECK_ID: &str = "EMIR.PSC.LIKELY_REJECTION_PATTERN";

/// Predicate that returns true when a record looks likely to fail a
/// given canonical check.
type Predicate = fn(&EmirRecord) -> bool;

fn predicate_for(check_id: &str) -> Option<Predicate> {
    match check_id {
        "EMIR.COMP.UTI_MISSING" => Some(uti_missing),
        "EMIR.COMP.NOTIONAL_CURRENCY_MISSING" => Some(notional_ccy_missing),
        "EMIR.COMP.VALUATION_MISSING" => Some(valuation_missing),
        "EMIR.COMP.COUNTERPARTY_1_MISSING" => Some(cp1_missing),
        "EMIR.COMP.COUNTERPARTY_2_MISSING" => Some(cp2_missing),
        "EMIR.ACC.NEGATIVE_NOTIONAL" => Some(negative_notional),
        "EMIR.ACC.ZERO_NOTIONAL" => Some(zero_notional),
        _ => None,
    }
}

fn uti_missing(r: &EmirRecord) -> bool {
    r.uti.as_deref().map(str::trim).unwrap_or("").is_empty()
}
fn notional_ccy_missing(r: &EmirRecord) -> bool {
    r.notional_amount.is_some()
        && r.notional_currency
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .is_empty()
}
fn valuation_missing(r: &EmirRecord) -> bool {
    r.valuation_amount.is_none()
        && r.termination_date.is_none()
        && !r.uti.as_deref().map(str::trim).unwrap_or("").is_empty()
}
fn cp1_missing(r: &EmirRecord) -> bool {
    r.counterparty_1
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
}
fn cp2_missing(r: &EmirRecord) -> bool {
    r.counterparty_2
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
}
fn negative_notional(r: &EmirRecord) -> bool {
    r.notional_amount
        .map(|n| n.is_sign_negative())
        .unwrap_or(false)
}
fn zero_notional(r: &EmirRecord) -> bool {
    r.notional_amount.map(|n| n.is_zero()).unwrap_or(false)
}

impl PreSubmissionCheck for EmirPscLikelyRejectionPattern {
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
        records: &[EmirRecord],
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
                    regime: Regime::Emir,
                    severity: Severity::Warning,
                    dimension: DqDimension::Validity,
                    record_id: r.record_id.clone(),
                    uti: r.uti.clone(),
                    field: None,
                    value: Some(cause.reason_code.clone()),
                    message: format!(
                        "Record matches {suggested} — historically the TR's #{rank} rejection cause ({cnt}/{total}, {share:.1}%, reason_code={code}).",
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
    fn flags_records_missing_notional_ccy_when_cause_is_top() {
        use rust_decimal::Decimal;
        let p = profile(100, &[("VAL01", 50, "EMIR.COMP.NOTIONAL_CURRENCY_MISSING")]);
        let records = vec![
            EmirRecord {
                record_id: Some("ok".into()),
                notional_amount: Some(Decimal::from(1000)),
                notional_currency: Some("EUR".into()),
                ..Default::default()
            },
            EmirRecord {
                record_id: Some("bad".into()),
                notional_amount: Some(Decimal::from(1000)),
                notional_currency: None,
                ..Default::default()
            },
        ];
        let ctx = CheckContext::now_with_defaults();
        let issues = EmirPscLikelyRejectionPattern.run(&records, &p, &ctx);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].record_id.as_deref(), Some("bad"));
        assert!(issues[0].message.contains("50.0%"));
    }

    #[test]
    fn skips_causes_without_predicate_mapping() {
        let p = profile(100, &[("XYZ", 50, "EMIR.UNKNOWN.CHECK")]);
        let records = vec![EmirRecord {
            uti: None,
            ..Default::default()
        }];
        let ctx = CheckContext::now_with_defaults();
        assert!(EmirPscLikelyRejectionPattern
            .run(&records, &p, &ctx)
            .is_empty());
    }

    #[test]
    fn empty_profile_yields_nothing() {
        let p = RejectionProfile::default();
        let records = vec![EmirRecord {
            uti: None,
            ..Default::default()
        }];
        let ctx = CheckContext::now_with_defaults();
        assert!(EmirPscLikelyRejectionPattern
            .run(&records, &p, &ctx)
            .is_empty());
    }
}
