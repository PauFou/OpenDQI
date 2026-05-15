//! Cross-layer TR-audit coherence checks — pure functions shared by
//! the CLI (`opendqi {emir,sftr} tr-audit`) and the local web UI.
//!
//! The per-layer checks (TAR / TSR / feedback / activity) are run by
//! the existing `run_all_*` registries; this module only computes the
//! three `*.AUD.*` cross-layer coherence checks that need TAR + TSR +
//! feedback together:
//!
//! - `*.AUD.NEWT_IN_TAR_NOT_IN_TSR` (high)
//! - `*.AUD.OUTSTANDING_IN_TSR_NOT_IN_TAR` (warning)
//! - `*.AUD.REJECTED_BUT_OUTSTANDING_IN_TSR` (critical)

use std::collections::HashSet;

use crate::model::{
    DqDimension, DqIssue, EmirRecord, FeedbackRecord, FeedbackType, Regime, Severity, SftrRecord,
    SftrTrStateRecord, TrStateRecord,
};

fn trimmed_nonempty(s: Option<&str>) -> Option<&str> {
    s.map(str::trim).filter(|u| !u.is_empty())
}

fn is_outstanding(status: Option<&str>) -> bool {
    status
        .map(|s| {
            let s = s.trim();
            s.is_empty()
                || s.eq_ignore_ascii_case("OUTSTANDING")
                || s.eq_ignore_ascii_case("ACTIVE")
        })
        .unwrap_or(true)
}

#[allow(clippy::too_many_arguments)]
fn aud_issue(
    check_id: &str,
    regime: Regime,
    severity: Severity,
    record_id: Option<String>,
    uti: &str,
    message: String,
    source_file: Option<String>,
) -> DqIssue {
    DqIssue {
        check_id: check_id.into(),
        regime,
        severity,
        dimension: DqDimension::Consistency,
        record_id,
        uti: Some(uti.to_owned()),
        field: Some("uti".into()),
        value: Some(uti.to_owned()),
        message,
        source_file,
        evidence: Vec::new(),
    }
}

/// EMIR cross-layer TR-audit checks. `tsr_source` / `feedback_source`
/// are the origin labels used for the issues that aren't tied to a
/// single TAR record.
pub fn compute_tr_audit_emir_issues(
    tar: &[EmirRecord],
    tsr: &[TrStateRecord],
    feedback: &[FeedbackRecord],
    tsr_source: &str,
    feedback_source: &str,
) -> Vec<DqIssue> {
    let tsr_utis: HashSet<&str> = tsr
        .iter()
        .filter_map(|r| trimmed_nonempty(r.uti.as_deref()))
        .collect();
    let tar_utis: HashSet<&str> = tar
        .iter()
        .filter_map(|r| trimmed_nonempty(r.uti.as_deref()))
        .collect();
    let rejected_utis: HashSet<&str> = feedback
        .iter()
        .filter(|f| matches!(f.feedback_type, FeedbackType::Rejected))
        .filter_map(|f| trimmed_nonempty(f.uti.as_deref()))
        .collect();
    let tsr_outstanding_utis: HashSet<&str> = tsr
        .iter()
        .filter(|r| is_outstanding(r.status.as_deref()))
        .filter_map(|r| trimmed_nonempty(r.uti.as_deref()))
        .collect();

    let mut out = Vec::new();

    for r in tar {
        let is_newt = r
            .action_type
            .as_deref()
            .map(|a| a.eq_ignore_ascii_case("NEWT"))
            .unwrap_or(false);
        if !is_newt {
            continue;
        }
        if let Some(uti) = trimmed_nonempty(r.uti.as_deref()) {
            if !tsr_utis.contains(uti) {
                out.push(aud_issue(
                    "EMIR.AUD.NEWT_IN_TAR_NOT_IN_TSR",
                    Regime::Emir,
                    Severity::High,
                    r.record_id.clone(),
                    uti,
                    format!(
                        "UTI {uti} was NEWT'd in the TAR but is absent from the TSR — submission may not have been accepted."
                    ),
                    r.source_file.clone(),
                ));
            }
        }
    }
    for uti in tsr_outstanding_utis.iter() {
        if !tar_utis.contains(uti) {
            out.push(aud_issue(
                "EMIR.AUD.OUTSTANDING_IN_TSR_NOT_IN_TAR",
                Regime::Emir,
                Severity::Warning,
                None,
                uti,
                format!(
                    "UTI {uti} is outstanding in the TSR but no record appears in the TAR for this period."
                ),
                Some(tsr_source.to_owned()),
            ));
        }
    }
    for uti in rejected_utis.iter() {
        if tsr_outstanding_utis.contains(uti) {
            out.push(aud_issue(
                "EMIR.AUD.REJECTED_BUT_OUTSTANDING_IN_TSR",
                Regime::Emir,
                Severity::Critical,
                None,
                uti,
                format!(
                    "UTI {uti} is reported rejected in the feedback file yet appears outstanding in the TSR — TR-side inconsistency."
                ),
                Some(feedback_source.to_owned()),
            ));
        }
    }
    out
}

/// SFTR cross-layer TR-audit checks (mirror of the EMIR variant).
pub fn compute_tr_audit_sftr_issues(
    tar: &[SftrRecord],
    tsr: &[SftrTrStateRecord],
    feedback: &[FeedbackRecord],
    tsr_source: &str,
    feedback_source: &str,
) -> Vec<DqIssue> {
    let tsr_utis: HashSet<&str> = tsr
        .iter()
        .filter_map(|r| trimmed_nonempty(r.uti.as_deref()))
        .collect();
    let tar_utis: HashSet<&str> = tar
        .iter()
        .filter_map(|r| trimmed_nonempty(r.uti.as_deref()))
        .collect();
    let rejected_utis: HashSet<&str> = feedback
        .iter()
        .filter(|f| matches!(f.feedback_type, FeedbackType::Rejected))
        .filter_map(|f| trimmed_nonempty(f.uti.as_deref()))
        .collect();
    let tsr_outstanding_utis: HashSet<&str> = tsr
        .iter()
        .filter(|r| is_outstanding(r.status.as_deref()))
        .filter_map(|r| trimmed_nonempty(r.uti.as_deref()))
        .collect();

    let mut out = Vec::new();

    for r in tar {
        let is_newt = r
            .action_type
            .as_deref()
            .map(|a| a.eq_ignore_ascii_case("NEWT"))
            .unwrap_or(false);
        if !is_newt {
            continue;
        }
        if let Some(uti) = trimmed_nonempty(r.uti.as_deref()) {
            if !tsr_utis.contains(uti) {
                out.push(aud_issue(
                    "SFTR.AUD.NEWT_IN_TAR_NOT_IN_TSR",
                    Regime::Sftr,
                    Severity::High,
                    r.record_id.clone(),
                    uti,
                    format!(
                        "SFT UTI {uti} was NEWT'd in the TAR but is absent from the TSR — submission may not have been accepted."
                    ),
                    r.source_file.clone(),
                ));
            }
        }
    }
    for uti in tsr_outstanding_utis.iter() {
        if !tar_utis.contains(uti) {
            out.push(aud_issue(
                "SFTR.AUD.OUTSTANDING_IN_TSR_NOT_IN_TAR",
                Regime::Sftr,
                Severity::Warning,
                None,
                uti,
                format!(
                    "SFT UTI {uti} is outstanding in the TSR but no record appears in the TAR for this period."
                ),
                Some(tsr_source.to_owned()),
            ));
        }
    }
    for uti in rejected_utis.iter() {
        if tsr_outstanding_utis.contains(uti) {
            out.push(aud_issue(
                "SFTR.AUD.REJECTED_BUT_OUTSTANDING_IN_TSR",
                Regime::Sftr,
                Severity::Critical,
                None,
                uti,
                format!(
                    "SFT UTI {uti} is reported rejected in the feedback file yet appears outstanding in the TSR — TR-side inconsistency."
                ),
                Some(feedback_source.to_owned()),
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emir_rejected_but_outstanding_fires_critical() {
        let tar = vec![EmirRecord {
            uti: Some("U-NEWT-MISSING".into()),
            action_type: Some("NEWT".into()),
            record_id: Some("r1".into()),
            ..Default::default()
        }];
        let tsr = vec![TrStateRecord {
            uti: Some("U-REJ".into()),
            status: Some("OUTSTANDING".into()),
            ..Default::default()
        }];
        let feedback = vec![FeedbackRecord {
            uti: Some("U-REJ".into()),
            feedback_type: FeedbackType::Rejected,
            ..Default::default()
        }];
        let issues = compute_tr_audit_emir_issues(&tar, &tsr, &feedback, "tsr.xml", "fb.xml");
        assert!(issues
            .iter()
            .any(|i| i.check_id == "EMIR.AUD.NEWT_IN_TAR_NOT_IN_TSR"));
        let crit = issues
            .iter()
            .find(|i| i.check_id == "EMIR.AUD.REJECTED_BUT_OUTSTANDING_IN_TSR")
            .expect("critical AUD issue");
        assert_eq!(crit.severity, Severity::Critical);
        assert_eq!(crit.uti.as_deref(), Some("U-REJ"));
    }

    #[test]
    fn clean_inputs_emit_nothing() {
        let tar = vec![EmirRecord {
            uti: Some("U1".into()),
            action_type: Some("NEWT".into()),
            ..Default::default()
        }];
        let tsr = vec![TrStateRecord {
            uti: Some("U1".into()),
            status: Some("OUTSTANDING".into()),
            ..Default::default()
        }];
        let issues = compute_tr_audit_emir_issues(&tar, &tsr, &[], "t", "f");
        assert!(issues.is_empty(), "got {issues:?}");
    }
}
