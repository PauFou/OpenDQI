//! SFTR.VLD.GMRA_GMSLA_VERSION_PLAUSIBLE — for GMRA (repo) and GMSLA
//! (securities lending) master agreements, the published version is
//! among the publicly known editions. Other master agreement types
//! are not checked here.

use super::SftrCheck;
use crate::dq::formats::is_in;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};

/// Check implementation.
pub struct SftrGmraGmslaVersionPlausible;

const CHECK_ID: &str = "SFTR.VLD.GMRA_GMSLA_VERSION_PLAUSIBLE";
const GMRA_VERSIONS: &[&str] = &["1995", "2000", "2011"];
const GMSLA_VERSIONS: &[&str] = &["2000", "2010", "2018"];

impl SftrCheck for SftrGmraGmslaVersionPlausible {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Validity
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[SftrRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter_map(|r| {
                let tp = r.master_agreement_type.as_deref()?.trim();
                let v = r.master_agreement_version.as_deref()?.trim();
                if v.is_empty() {
                    return None;
                }
                let allowed: &[&str] = if tp.eq_ignore_ascii_case("GMRA") {
                    GMRA_VERSIONS
                } else if tp.eq_ignore_ascii_case("GMSLA") {
                    GMSLA_VERSIONS
                } else {
                    return None;
                };
                if is_in(v, allowed) {
                    None
                } else {
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Sftr,
                        severity: Severity::Warning,
                        dimension: DqDimension::Validity,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("master_agreement_version".into()),
                        value: Some(v.to_owned()),
                        message: format!("{tp} version '{v}' is not a known published edition."),
                        source_file: r.source_file.clone(),
                    })
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flags_unknown_gmra_version() {
        let r = SftrRecord {
            master_agreement_type: Some("GMRA".into()),
            master_agreement_version: Some("1999".into()),
            ..Default::default()
        };
        assert_eq!(
            SftrGmraGmslaVersionPlausible
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn accepts_known_gmsla() {
        let r = SftrRecord {
            master_agreement_type: Some("GMSLA".into()),
            master_agreement_version: Some("2010".into()),
            ..Default::default()
        };
        assert!(SftrGmraGmslaVersionPlausible
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
    #[test]
    fn ignores_other_type() {
        let r = SftrRecord {
            master_agreement_type: Some("ISDA".into()),
            master_agreement_version: Some("2010".into()),
            ..Default::default()
        };
        assert!(SftrGmraGmslaVersionPlausible
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
