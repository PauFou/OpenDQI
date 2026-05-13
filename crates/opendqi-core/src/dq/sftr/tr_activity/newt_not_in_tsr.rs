//! SFTR.TRA.NEWT_NOT_IN_TSR — UTI NEWT'd in TAR but absent from
//! the companion TSR (only when `--tsr` is provided).

use std::collections::HashSet;

use super::SftrTrActivityCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord, SftrTrStateRecord};

/// Check implementation.
pub struct SftrNewtNotInTsr;

const CHECK_ID: &str = "SFTR.TRA.NEWT_NOT_IN_TSR";

impl SftrTrActivityCheck for SftrNewtNotInTsr {
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
        _prior: &[SftrRecord],
        tsr: Option<&[SftrTrStateRecord]>,
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        let tsr = match tsr {
            Some(t) => t,
            None => return Vec::new(),
        };
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
                    regime: Regime::Sftr,
                    severity: Severity::High,
                    dimension: DqDimension::Consistency,
                    record_id: r.record_id.clone(),
                    uti: Some(uti.to_owned()),
                    field: Some("uti".into()),
                    value: Some(uti.to_owned()),
                    message: format!(
                        "UTI {uti} is NEWT in the SFTR TAR but absent from the companion TSR."
                    ),
                    source_file: r.source_file.clone(),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flags_when_missing() {
        let tar = vec![SftrRecord {
            uti: Some("U1".into()),
            action_type: Some("NEWT".into()),
            ..Default::default()
        }];
        let tsr: Vec<SftrTrStateRecord> = vec![];
        assert_eq!(
            SftrNewtNotInTsr
                .run(&tar, &[], Some(&tsr), &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn no_tsr_no_check() {
        let tar = vec![SftrRecord {
            uti: Some("U1".into()),
            action_type: Some("NEWT".into()),
            ..Default::default()
        }];
        assert!(SftrNewtNotInTsr
            .run(&tar, &[], None, &CheckContext::now_with_defaults())
            .is_empty());
    }
}
