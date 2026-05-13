//! SFTR.VLD.ISIN_COLLATERAL — collateral ISIN shape (ISO 6166).

use super::SftrCheck;
use crate::dq::formats::is_valid_isin;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};

/// Check implementation.
pub struct SftrIsinCollateral;

const CHECK_ID: &str = "SFTR.VLD.ISIN_COLLATERAL";

impl SftrCheck for SftrIsinCollateral {
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
                let isin = r.collateral_isin.as_deref()?.trim();
                if isin.is_empty() || is_valid_isin(isin) {
                    None
                } else {
                    Some(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Sftr,
                        severity: Severity::Warning,
                        dimension: DqDimension::Validity,
                        record_id: r.record_id.clone(),
                        uti: r.uti.clone(),
                        field: Some("collateral_isin".into()),
                        value: Some(isin.to_owned()),
                        message: format!(
                            "Collateral ISIN '{isin}' is not a valid ISO 6166 identifier (2 letters + 9 alphanumeric + 1 digit)."
                        ),
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
    fn flags_invalid_isin() {
        let records = vec![
            SftrRecord {
                collateral_isin: Some("DE0001135275".into()),
                ..Default::default()
            },
            SftrRecord {
                collateral_isin: Some("NOTANISIN".into()),
                ..Default::default()
            },
        ];
        let ctx = CheckContext::now_with_defaults();
        let issues = SftrIsinCollateral.run(&records, &ctx);
        assert_eq!(issues.len(), 1);
    }
}
