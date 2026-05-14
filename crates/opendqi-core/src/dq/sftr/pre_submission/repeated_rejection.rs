//! SFTR.PSC.REPEATED_REJECTION — flag any SFT record whose UTI has
//! been rejected at least `RejectionProfile::repeated_rejected_utis[].count`
//! times by the TR in the analytics window. SFTR mirror of
//! `EMIR.PSC.REPEATED_REJECTION`.

use std::collections::HashMap;

use super::SftrPreSubmissionCheck;
use crate::dq::CheckContext;
use crate::model::{
    DqDimension, DqIssue, EvidenceItem, Regime, RejectionProfile, Severity, SftrRecord,
};

/// Check implementation.
pub struct SftrPscRepeatedRejection;

const CHECK_ID: &str = "SFTR.PSC.REPEATED_REJECTION";

impl SftrPreSubmissionCheck for SftrPscRepeatedRejection {
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
        records: &[SftrRecord],
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
                    regime: Regime::Sftr,
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
            SftrRecord {
                uti: Some("U-GOOD".into()),
                record_id: Some("1".into()),
                ..Default::default()
            },
            SftrRecord {
                uti: Some("U-BAD".into()),
                record_id: Some("2".into()),
                ..Default::default()
            },
            SftrRecord {
                uti: Some("U-WORSE".into()),
                record_id: Some("3".into()),
                ..Default::default()
            },
        ];
        let ctx = CheckContext::now_with_defaults();
        let issues = SftrPscRepeatedRejection.run(&records, &profile, &ctx);
        assert_eq!(issues.len(), 2);
        // Evidence carries the prior rejection count.
        assert!(issues.iter().all(|i| !i.evidence.is_empty()));
    }

    #[test]
    fn empty_profile_yields_no_issues() {
        let profile = RejectionProfile::default();
        let records = vec![SftrRecord {
            uti: Some("U-ANY".into()),
            ..Default::default()
        }];
        let ctx = CheckContext::now_with_defaults();
        assert!(SftrPscRepeatedRejection
            .run(&records, &profile, &ctx)
            .is_empty());
    }
}
