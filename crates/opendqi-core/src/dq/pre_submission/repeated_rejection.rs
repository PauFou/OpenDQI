//! EMIR.PSC.REPEATED_REJECTION — flag any record whose UTI has been
//! rejected at least `RejectionProfile::repeated_rejected_utis[].count`
//! times by the TR in the analytics window. The firm is about to
//! re-submit a trade the TR has already rejected repeatedly — the
//! pre-submission check escalates to `High` severity to block the
//! re-submission until the underlying cause is fixed.

use std::collections::HashMap;

use super::PreSubmissionCheck;
use crate::dq::CheckContext;
use crate::model::{
    DqDimension, DqIssue, EmirRecord, EvidenceItem, Regime, RejectionProfile, Severity,
};

/// Check implementation.
pub struct EmirPscRepeatedRejection;

const CHECK_ID: &str = "EMIR.PSC.REPEATED_REJECTION";

impl PreSubmissionCheck for EmirPscRepeatedRejection {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(
        &self,
        records: &[EmirRecord],
        profile: &RejectionProfile,
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        if profile.repeated_rejected_utis.is_empty() {
            return Vec::new();
        }
        let by_uti: HashMap<&str, u64> = profile
            .repeated_rejected_utis
            .iter()
            .map(|r| (r.uti.as_str(), r.count))
            .collect();
        records
            .iter()
            .filter_map(|r| {
                let uti = r.uti.as_deref()?.trim();
                let count = by_uti.get(uti)?;
                Some(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Emir,
                    severity: Severity::High,
                    dimension: DqDimension::Consistency,
                    record_id: r.record_id.clone(),
                    uti: Some(uti.to_owned()),
                    field: Some("uti".into()),
                    value: Some(uti.to_owned()),
                    message: format!(
                        "UTI {uti} has been rejected {count} time(s) by the TR in the analytics window. Investigate before re-submitting."
                    ),
                    source_file: r.source_file.clone(),
                    evidence: vec![EvidenceItem {
                        field: "rejection_count".into(),
                        before: None,
                        after: Some(count.to_string()),
                        source_line: None,
                    }],
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{RejectionProfile, RepeatedRejection};

    fn profile_with(utis: &[(&str, u64)]) -> RejectionProfile {
        RejectionProfile {
            repeated_rejected_utis: utis
                .iter()
                .map(|(u, n)| RepeatedRejection {
                    uti: (*u).to_owned(),
                    count: *n,
                })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn flags_records_whose_uti_is_in_the_repeated_set() {
        let profile = profile_with(&[("U-BAD", 5), ("U-WORSE", 12)]);
        let records = vec![
            EmirRecord {
                uti: Some("U-GOOD".into()),
                record_id: Some("1".into()),
                ..Default::default()
            },
            EmirRecord {
                uti: Some("U-BAD".into()),
                record_id: Some("2".into()),
                ..Default::default()
            },
            EmirRecord {
                uti: Some("U-WORSE".into()),
                record_id: Some("3".into()),
                ..Default::default()
            },
        ];
        let ctx = CheckContext::now_with_defaults();
        let issues = EmirPscRepeatedRejection.run(&records, &profile, &ctx);
        assert_eq!(issues.len(), 2);
        let utis: Vec<&str> = issues.iter().filter_map(|i| i.uti.as_deref()).collect();
        assert!(utis.contains(&"U-BAD"));
        assert!(utis.contains(&"U-WORSE"));
        // Each issue carries the prior rejection count as evidence.
        assert!(issues.iter().all(|i| !i.evidence.is_empty()));
    }

    #[test]
    fn empty_profile_yields_no_issues() {
        let profile = RejectionProfile::default();
        let records = vec![EmirRecord {
            uti: Some("U-ANY".into()),
            ..Default::default()
        }];
        let ctx = CheckContext::now_with_defaults();
        assert!(EmirPscRepeatedRejection
            .run(&records, &profile, &ctx)
            .is_empty());
    }
}
