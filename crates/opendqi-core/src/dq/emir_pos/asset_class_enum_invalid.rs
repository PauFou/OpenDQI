//! `EMIR.POS.ASSET_CLASS_ENUM_INVALID` — the position-set
//! record carries an `asset_class` value outside the ESMA
//! `ProductType4Code` enum.
//!
//! Per the auth.090 XSD, AsstClss is bound to
//! `ProductType4Code` whose members are the 5 EMIR-recognised
//! asset classes: CRDT (credit), CURR (currency), EQUI
//! (equity), INTR (interest rates), COMM (commodities) — plus
//! OTHR as a catch-all in some variants. Anything else is a
//! data-quality defect that breaks downstream cross-asset-class
//! aggregation. None (asset_class absent) doesn't fire — the
//! E5 design surfaces only *invalid* values, not *missing*
//! ones (the absence case is covered by a separate completeness
//! DQI if needed).

use super::EmirPosCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, EmirPositionSetRecord, Regime, Severity};

/// Check implementation.
pub struct EmirPosAssetClassEnumInvalid;

const CHECK_ID: &str = "EMIR.POS.ASSET_CLASS_ENUM_INVALID";

/// 5 ESMA ProductType4Code members + OTHR per the v1.0 XSD.
const VALID_ASSET_CLASSES: &[&str] = &["CRDT", "CURR", "EQUI", "INTR", "COMM", "OTHR"];

fn is_valid(c: &str) -> bool {
    VALID_ASSET_CLASSES.contains(&c)
}

impl EmirPosCheck for EmirPosAssetClassEnumInvalid {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Validity
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[EmirPositionSetRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        let mut out = Vec::new();
        for r in records {
            if let Some(c) = r.asset_class.as_deref() {
                if !is_valid(c) {
                    out.push(DqIssue {
                        check_id: CHECK_ID.into(),
                        regime: Regime::Emir,
                        severity: Severity::High,
                        dimension: DqDimension::Validity,
                        record_id: r.record_id.clone(),
                        uti: None,
                        field: Some("asset_class".into()),
                        value: Some(c.to_string()),
                        message: format!(
                            "asset_class {c:?} is not in the ESMA ProductType4Code enum \
                             (CRDT/CURR/EQUI/INTR/COMM/OTHR); record breaks downstream \
                             cross-asset-class aggregation"
                        ),
                        source_file: r.source_file.clone(),
                        evidence: Vec::new(),
                    });
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> CheckContext {
        CheckContext {
            thresholds: Default::default(),
            today: chrono::NaiveDate::from_ymd_opt(2026, 5, 21).unwrap(),
            now: chrono::DateTime::parse_from_rfc3339("2026-05-21T00:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
        }
    }

    #[test]
    fn fires_on_unknown_asset_class() {
        let r = EmirPositionSetRecord {
            record_id: Some("R-BOGUS".into()),
            asset_class: Some("CRYPTO".into()),
            ..Default::default()
        };
        let out = EmirPosAssetClassEnumInvalid.run(&[r], &ctx());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].severity, Severity::High);
    }

    #[test]
    fn does_not_fire_when_asset_class_is_none() {
        let r = EmirPositionSetRecord {
            asset_class: None,
            ..Default::default()
        };
        let out = EmirPosAssetClassEnumInvalid.run(&[r], &ctx());
        assert!(out.is_empty());
    }

    #[test]
    fn does_not_fire_on_each_of_6_valid_codes() {
        for c in VALID_ASSET_CLASSES {
            let r = EmirPositionSetRecord {
                asset_class: Some((*c).to_string()),
                ..Default::default()
            };
            let out = EmirPosAssetClassEnumInvalid.run(&[r], &ctx());
            assert!(out.is_empty(), "{c} should be accepted");
        }
    }
}
