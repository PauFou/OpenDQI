//! EMIR.VLD.CREDIT_SECTOR_ENUM — for credit-class trades the
//! `underlying_id` should look like an ISIN (12 chars) or one of the
//! well-known credit index families.

use super::{Check, CheckContext};
use crate::dq::formats::{is_in, is_valid_isin};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct CreditSectorEnum;

const CHECK_ID: &str = "EMIR.VLD.CREDIT_SECTOR_ENUM";
const CREDIT_ASSET_CLASSES: &[&str] = &["CR", "CRDT"];
const CREDIT_INDEX_FAMILIES: &[&str] = &[
    "ITRAXX",
    "ITRAXX-EUROPE",
    "ITRAXX-CROSSOVER",
    "ITRAXX-SENFIN",
    "CDX",
    "CDX-IG",
    "CDX-HY",
    "CDX-EM",
];

impl Check for CreditSectorEnum {
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
                let asset_class = r.asset_class.as_deref()?.trim();
                if !is_in(asset_class, CREDIT_ASSET_CLASSES) {
                    return None;
                }
                let underlying = r.underlying_id.as_deref()?.trim();
                if underlying.is_empty() {
                    return None;
                }
                if is_valid_isin(underlying) {
                    return None;
                }
                let upper = underlying.to_ascii_uppercase();
                if CREDIT_INDEX_FAMILIES
                    .iter()
                    .any(|fam| upper.starts_with(fam))
                {
                    return None;
                }
                Some(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Emir,
                    severity: Severity::Warning,
                    dimension: DqDimension::Validity,
                    record_id: r.record_id.clone(),
                    uti: r.uti.clone(),
                    field: Some("underlying_id".into()),
                    value: Some(underlying.to_owned()),
                    message: format!(
                        "Credit underlying_id '{underlying}' is neither a valid ISIN nor a recognised credit index family."
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
    fn flags_garbage_underlying() {
        let r = EmirRecord {
            asset_class: Some("CR".into()),
            underlying_id: Some("FOOBAR".into()),
            ..Default::default()
        };
        assert_eq!(
            CreditSectorEnum
                .run(&[r], &CheckContext::now_with_defaults())
                .len(),
            1
        );
    }

    #[test]
    fn accepts_isin() {
        let r = EmirRecord {
            asset_class: Some("CR".into()),
            underlying_id: Some("DE0001135275".into()),
            ..Default::default()
        };
        assert!(CreditSectorEnum
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }

    #[test]
    fn accepts_itraxx_family() {
        let r = EmirRecord {
            asset_class: Some("CRDT".into()),
            underlying_id: Some("ITRAXX-EUROPE-S40".into()),
            ..Default::default()
        };
        assert!(CreditSectorEnum
            .run(&[r], &CheckContext::now_with_defaults())
            .is_empty());
    }
}
