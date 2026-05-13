//! EMIR.VLD.ISDA_VERSION_PLAUSIBLE — for ISDA master agreements, the
//! published version is one of `{1992, 2002, 2017}`.

use super::{Check, CheckContext};
use crate::dq::formats::is_in;
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct IsdaVersionPlausible;

const CHECK_ID: &str = "EMIR.VLD.ISDA_VERSION_PLAUSIBLE";
const ALLOWED_VERSIONS: &[&str] = &["1992", "2002", "2017"];

impl Check for IsdaVersionPlausible {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Validity
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        records
            .iter()
            .filter_map(|r| {
                let tp = r.master_agreement_type.as_deref()?;
                if !tp.eq_ignore_ascii_case("ISDA") {
                    return None;
                }
                let v = r.master_agreement_version.as_deref()?.trim();
                if v.is_empty() || is_in(v, ALLOWED_VERSIONS) {
                    None
                } else {
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Emir,
                        severity: Severity::Warning,
                        dimension: DqDimension::Validity,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("master_agreement_version".into()),
                        value: Some(v.to_owned()),
                        message: format!("ISDA version '{v}' is not one of {{1992, 2002, 2017}}."),
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
    fn flags_unknown_isda_version() {
        let r = EmirRecord {
            master_agreement_type: Some("ISDA".into()),
            master_agreement_version: Some("2010".into()),
            ..Default::default()
        };
        assert_eq!(
            IsdaVersionPlausible
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }
    #[test]
    fn accepts_known() {
        let r = EmirRecord {
            master_agreement_type: Some("ISDA".into()),
            master_agreement_version: Some("2002".into()),
            ..Default::default()
        };
        assert!(IsdaVersionPlausible
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
    #[test]
    fn ignores_non_isda() {
        let r = EmirRecord {
            master_agreement_type: Some("GMRA".into()),
            master_agreement_version: Some("2011".into()),
            ..Default::default()
        };
        assert!(IsdaVersionPlausible
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
