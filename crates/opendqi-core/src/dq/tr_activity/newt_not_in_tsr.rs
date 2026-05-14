//! EMIR.TRA.NEWT_NOT_IN_TSR — UTI carries a `NEWT` action in the
//! TAR batch but is absent from the companion TSR. Indicates the
//! trade may not have been accepted by the TR despite the
//! submission. Only fires when both TAR and TSR are provided.

use std::collections::HashSet;

use crate::dq::{CheckContext, TrActivityCheck};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity, TrStateRecord};

/// Check implementation.
pub struct EmirNewtNotInTsr;

const CHECK_ID: &str = "EMIR.TRA.NEWT_NOT_IN_TSR";

impl TrActivityCheck for EmirNewtNotInTsr {
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
        _prior: &[EmirRecord],
        tsr: Option<&[TrStateRecord]>,
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        let tsr = match tsr {
            Some(t) => t,
            None => return Vec::new(),
        };
        // Build a set of UTIs the TSR knows about (any status).
        let tsr_utis: HashSet<&str> = tsr
            .iter()
            .filter_map(|r| r.uti.as_deref())
            .map(str::trim)
            .filter(|u| !u.is_empty())
            .collect();
        records
            .iter()
            .filter_map(|r| {
                let is_newt = r
                    .action_type
                    .as_deref()
                    .map(|a| a.eq_ignore_ascii_case("NEWT"))
                    .unwrap_or(false);
                if !is_newt {
                    return None;
                }
                let uti = r.uti.as_deref()?.trim();
                if uti.is_empty() || tsr_utis.contains(uti) {
                    return None;
                }
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
                        "UTI {uti} appears as NEWT in the TAR batch but is absent from the companion TSR — submission may not have been accepted."
                    ),
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

    #[test]
    fn flags_when_uti_missing_in_tsr() {
        let tar = vec![EmirRecord {
            uti: Some("U1".into()),
            action_type: Some("NEWT".into()),
            ..Default::default()
        }];
        let tsr = vec![TrStateRecord {
            uti: Some("U2".into()),
            ..Default::default()
        }];
        assert_eq!(
            EmirNewtNotInTsr
                .run(&tar, &[], Some(&tsr), &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }

    #[test]
    fn no_tsr_means_no_check() {
        let tar = vec![EmirRecord {
            uti: Some("U1".into()),
            action_type: Some("NEWT".into()),
            ..Default::default()
        }];
        assert!(EmirNewtNotInTsr
            .run(&tar, &[], None, &CheckContext::now_with_defaults())
            .is_empty());
    }

    #[test]
    fn accepts_uti_present_in_tsr() {
        let tar = vec![EmirRecord {
            uti: Some("U1".into()),
            action_type: Some("NEWT".into()),
            ..Default::default()
        }];
        let tsr = vec![TrStateRecord {
            uti: Some("U1".into()),
            ..Default::default()
        }];
        assert!(EmirNewtNotInTsr
            .run(&tar, &[], Some(&tsr), &CheckContext::now_with_defaults())
            .is_empty());
    }
}
