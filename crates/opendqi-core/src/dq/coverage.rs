//! Gap-fill checks identified by the field-coverage audit
//! (`docs/field-coverage.md`). Each check covers a typed field that
//! was not previously exercised by `default_checks()` /
//! `default_sftr_checks()`.

use rust_decimal::Decimal;

use crate::dq::{Check, CheckContext, SftrCheck};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity, SftrRecord};

fn emir_issue(
    check_id: &str,
    severity: Severity,
    dimension: DqDimension,
    r: &EmirRecord,
    field: &str,
    value: Option<String>,
    message: String,
) -> DqIssue {
    DqIssue {
        check_id: check_id.into(),
        regime: Regime::Emir,
        severity,
        dimension,
        record_id: r.record_id.clone(),
        uti: r.uti.clone(),
        field: Some(field.into()),
        value,
        message,
        source_file: r.source_file.clone(),
        evidence: Vec::new(),
    }
}

fn sftr_issue(
    check_id: &str,
    severity: Severity,
    dimension: DqDimension,
    r: &SftrRecord,
    field: &str,
    value: Option<String>,
    message: String,
) -> DqIssue {
    DqIssue {
        check_id: check_id.into(),
        regime: Regime::Sftr,
        severity,
        dimension,
        record_id: r.record_id.clone(),
        uti: r.uti.clone(),
        field: Some(field.into()),
        value,
        message,
        source_file: r.source_file.clone(),
        evidence: Vec::new(),
    }
}

// =================================================================
// EMIR gap-fill checks
// =================================================================

const EMIR_CORPORATE_SECTORS: &[&str] = &[
    "ASMC", "BLA", "CDTI", "CRDT", "ESA", "ETI", "ICCD", "INHV", "INRE", "OTHR",
];

/// Check implementation.
pub struct EmirCorporateSectorEnum;

impl Check for EmirCorporateSectorEnum {
    fn id(&self) -> &'static str {
        "EMIR.VLD.CORPORATE_SECTOR_ENUM"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Validity
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        let mut out = Vec::new();
        for r in records {
            if let Some(s) = r.corporate_sector.as_deref() {
                let upper = s.trim().to_uppercase();
                if !upper.is_empty() && !EMIR_CORPORATE_SECTORS.contains(&upper.as_str()) {
                    out.push(emir_issue(
                        self.id(),
                        self.severity(),
                        self.dimension(),
                        r,
                        "corporate_sector",
                        Some(s.to_owned()),
                        format!("corporate_sector '{s}' is not in the EMIR enum."),
                    ));
                }
            }
        }
        out
    }
}

const EMIR_REPORTING_OBLIGATION: &[&str] = &["TRUE", "FALSE", "Y", "N", "1", "0"];

/// Check implementation.
pub struct EmirReportingObligationIndicatorEnum;

impl Check for EmirReportingObligationIndicatorEnum {
    fn id(&self) -> &'static str {
        "EMIR.VLD.REPORTING_OBLIGATION_INDICATOR_ENUM"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Validity
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        let mut out = Vec::new();
        for r in records {
            if let Some(s) = r.reporting_obligation_indicator.as_deref() {
                let upper = s.trim().to_uppercase();
                if !upper.is_empty() && !EMIR_REPORTING_OBLIGATION.contains(&upper.as_str()) {
                    out.push(emir_issue(
                        self.id(),
                        self.severity(),
                        self.dimension(),
                        r,
                        "reporting_obligation_indicator",
                        Some(s.to_owned()),
                        format!("reporting_obligation_indicator '{s}' is not boolean-like."),
                    ));
                }
            }
        }
        out
    }
}

/// Check implementation.
pub struct EmirPriceNegative;

impl Check for EmirPriceNegative {
    fn id(&self) -> &'static str {
        "EMIR.ACC.PRICE_NEGATIVE"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        let mut out = Vec::new();
        for r in records {
            if let Some(p) = r.price {
                if p < Decimal::ZERO {
                    // Negative prices are plausible for IR/CR
                    // (negative-yield bonds / coupons). Restrict the
                    // check to FX / EQ / CO where negative is
                    // genuinely suspicious.
                    let suspicious = r
                        .asset_class
                        .as_deref()
                        .map(|c| {
                            let u = c.trim().to_uppercase();
                            matches!(u.as_str(), "FX" | "CURR" | "EQ" | "EQUI" | "CO" | "COMM")
                        })
                        .unwrap_or(false);
                    if suspicious {
                        out.push(emir_issue(
                            self.id(),
                            self.severity(),
                            self.dimension(),
                            r,
                            "price",
                            Some(p.to_string()),
                            format!("price {p} is negative for asset class {:?}.", r.asset_class),
                        ));
                    }
                }
            }
        }
        out
    }
}

/// Check implementation.
pub struct EmirDeltaOutOfRange;

impl Check for EmirDeltaOutOfRange {
    fn id(&self) -> &'static str {
        "EMIR.ACC.DELTA_OUT_OF_RANGE"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        let mut out = Vec::new();
        let upper = Decimal::ONE;
        let lower = Decimal::NEGATIVE_ONE;
        for r in records {
            if let Some(d) = r.delta {
                if d < lower || d > upper {
                    out.push(emir_issue(
                        self.id(),
                        self.severity(),
                        self.dimension(),
                        r,
                        "delta",
                        Some(d.to_string()),
                        format!("delta {d} is outside [-1.0, 1.0]."),
                    ));
                }
            }
        }
        out
    }
}

/// Check implementation.
pub struct EmirGammaNegative;

impl Check for EmirGammaNegative {
    fn id(&self) -> &'static str {
        "EMIR.ACC.GAMMA_NEGATIVE"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        let mut out = Vec::new();
        for r in records {
            if let Some(g) = r.gamma {
                if g < Decimal::ZERO {
                    out.push(emir_issue(
                        self.id(),
                        self.severity(),
                        self.dimension(),
                        r,
                        "gamma",
                        Some(g.to_string()),
                        format!("gamma {g} is negative — unusual for long options."),
                    ));
                }
            }
        }
        out
    }
}

/// Check implementation.
pub struct EmirCommercialOrTreasuryRequiresNfc;

impl Check for EmirCommercialOrTreasuryRequiresNfc {
    fn id(&self) -> &'static str {
        "EMIR.CON.COMMERCIAL_OR_TREASURY_REQUIRES_NFC"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        let mut out = Vec::new();
        for r in records {
            if r.commercial_or_treasury_financing != Some(true) {
                continue;
            }
            let is_nfc = r
                .nature
                .as_deref()
                .map(|n| n.trim().to_uppercase().starts_with("NFC"))
                .unwrap_or(false);
            if !is_nfc {
                out.push(emir_issue(
                    self.id(),
                    self.severity(),
                    self.dimension(),
                    r,
                    "commercial_or_treasury_financing",
                    Some("true".into()),
                    "commercial_or_treasury_financing=true requires NFC nature (Article 10 EMIR)."
                        .into(),
                ));
            }
        }
        out
    }
}

// =================================================================
// SFTR gap-fill checks
// =================================================================

/// Check implementation.
pub struct SftrReuseIndicatorRequiresPortfolio;

impl SftrCheck for SftrReuseIndicatorRequiresPortfolio {
    fn id(&self) -> &'static str {
        "SFTR.CON.REUSE_INDICATOR_REQUIRES_PORTFOLIO"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[SftrRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        let mut out = Vec::new();
        for r in records {
            if r.reuse_indicator == Some(true) {
                let missing = r
                    .collateral_portfolio_code
                    .as_deref()
                    .map(|s| s.trim().is_empty())
                    .unwrap_or(true);
                if missing {
                    out.push(sftr_issue(
                        self.id(),
                        self.severity(),
                        self.dimension(),
                        r,
                        "collateral_portfolio_code",
                        None,
                        "reuse_indicator=true but no collateral_portfolio_code reported.".into(),
                    ));
                }
            }
        }
        out
    }
}

const SFTR_EVENT_TYPES: &[&str] = &[
    "TRAD", "MODI", "CORR", "ETRM", "VALU", "COLU", "REUU", "MARU", "EROR", "POSC",
];

/// Check implementation.
pub struct SftrEventTypeEnum;

impl SftrCheck for SftrEventTypeEnum {
    fn id(&self) -> &'static str {
        "SFTR.VLD.EVENT_TYPE_ENUM"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Validity
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[SftrRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        let mut out = Vec::new();
        for r in records {
            if let Some(e) = r.event_type.as_deref() {
                let upper = e.trim().to_uppercase();
                if !upper.is_empty() && !SFTR_EVENT_TYPES.contains(&upper.as_str()) {
                    out.push(sftr_issue(
                        self.id(),
                        self.severity(),
                        self.dimension(),
                        r,
                        "event_type",
                        Some(e.to_owned()),
                        format!("event_type '{e}' is not in the SFTR enum."),
                    ));
                }
            }
        }
        out
    }
}

const SFTR_MASTER_AGREEMENT_TYPES: &[&str] = &[
    "GMRA", "GMSLA", "ISDA", "CDEA", "EFET", "ICOM", "ISMA", "OTHR",
];

/// Check implementation.
pub struct SftrMasterAgreementTypeEnum;

impl SftrCheck for SftrMasterAgreementTypeEnum {
    fn id(&self) -> &'static str {
        "SFTR.VLD.MASTER_AGREEMENT_TYPE_ENUM"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Validity
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[SftrRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        let mut out = Vec::new();
        for r in records {
            if let Some(t) = r.master_agreement_type.as_deref() {
                let upper = t.trim().to_uppercase();
                if !upper.is_empty() && !SFTR_MASTER_AGREEMENT_TYPES.contains(&upper.as_str()) {
                    out.push(sftr_issue(
                        self.id(),
                        self.severity(),
                        self.dimension(),
                        r,
                        "master_agreement_type",
                        Some(t.to_owned()),
                        format!("master_agreement_type '{t}' is not in the SFTR enum."),
                    ));
                }
            }
        }
        out
    }
}

/// Check implementation.
pub struct SftrLendingFeeNegative;

impl SftrCheck for SftrLendingFeeNegative {
    fn id(&self) -> &'static str {
        "SFTR.ACC.LENDING_FEE_NEGATIVE"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[SftrRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        let mut out = Vec::new();
        for r in records {
            if let Some(f) = r.lending_fee {
                if f < Decimal::ZERO {
                    out.push(sftr_issue(
                        self.id(),
                        self.severity(),
                        self.dimension(),
                        r,
                        "lending_fee",
                        Some(f.to_string()),
                        format!("lending_fee {f} is negative."),
                    ));
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
            today: chrono::NaiveDate::from_ymd_opt(2026, 5, 13).unwrap(),
            now: chrono::DateTime::parse_from_rfc3339("2026-05-13T08:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
        }
    }

    #[test]
    fn corporate_sector_enum_flags_and_accepts() {
        let mut r = EmirRecord {
            corporate_sector: Some("XYZ".into()),
            ..Default::default()
        };
        assert_eq!(EmirCorporateSectorEnum.run(&[r.clone()], &ctx()).len(), 1);
        r.corporate_sector = Some("CRDT".into());
        assert!(EmirCorporateSectorEnum.run(&[r], &ctx()).is_empty());
    }

    #[test]
    fn reporting_obligation_flags_and_accepts() {
        let mut r = EmirRecord {
            reporting_obligation_indicator: Some("XYZ".into()),
            ..Default::default()
        };
        assert_eq!(
            EmirReportingObligationIndicatorEnum
                .run(&[r.clone()], &ctx())
                .len(),
            1
        );
        r.reporting_obligation_indicator = Some("TRUE".into());
        assert!(EmirReportingObligationIndicatorEnum
            .run(&[r], &ctx())
            .is_empty());
    }

    #[test]
    fn price_negative_only_for_relevant_asset_class() {
        let r = EmirRecord {
            price: Some(Decimal::from(-1)),
            asset_class: Some("FX".into()),
            ..Default::default()
        };
        assert_eq!(EmirPriceNegative.run(&[r], &ctx()).len(), 1);
        let r = EmirRecord {
            price: Some(Decimal::from(-1)),
            asset_class: Some("IR".into()),
            ..Default::default()
        };
        assert!(EmirPriceNegative.run(&[r], &ctx()).is_empty());
    }

    #[test]
    fn delta_out_of_range_flags_and_accepts() {
        let r = EmirRecord {
            delta: Some(Decimal::new(15, 1)),
            ..Default::default()
        };
        assert_eq!(EmirDeltaOutOfRange.run(&[r], &ctx()).len(), 1);
        let r = EmirRecord {
            delta: Some(Decimal::new(5, 1)),
            ..Default::default()
        };
        assert!(EmirDeltaOutOfRange.run(&[r], &ctx()).is_empty());
    }

    #[test]
    fn gamma_negative_flags_and_accepts() {
        let r = EmirRecord {
            gamma: Some(Decimal::from(-1)),
            ..Default::default()
        };
        assert_eq!(EmirGammaNegative.run(&[r], &ctx()).len(), 1);
        let r = EmirRecord {
            gamma: Some(Decimal::from(1)),
            ..Default::default()
        };
        assert!(EmirGammaNegative.run(&[r], &ctx()).is_empty());
    }

    #[test]
    fn commercial_treasury_requires_nfc_flags_and_accepts() {
        let mut r = EmirRecord {
            commercial_or_treasury_financing: Some(true),
            nature: Some("FC".into()),
            ..Default::default()
        };
        assert_eq!(
            EmirCommercialOrTreasuryRequiresNfc
                .run(&[r.clone()], &ctx())
                .len(),
            1
        );
        r.nature = Some("NFC".into());
        assert!(EmirCommercialOrTreasuryRequiresNfc
            .run(&[r], &ctx())
            .is_empty());
    }

    #[test]
    fn reuse_indicator_requires_portfolio_flags_and_accepts() {
        let mut r = SftrRecord {
            reuse_indicator: Some(true),
            ..Default::default()
        };
        assert_eq!(
            SftrReuseIndicatorRequiresPortfolio
                .run(&[r.clone()], &ctx())
                .len(),
            1
        );
        r.collateral_portfolio_code = Some("P".into());
        assert!(SftrReuseIndicatorRequiresPortfolio
            .run(&[r], &ctx())
            .is_empty());
    }

    #[test]
    fn sftr_event_type_enum_flags_and_accepts() {
        let mut r = SftrRecord {
            event_type: Some("ZZZ".into()),
            ..Default::default()
        };
        assert_eq!(SftrEventTypeEnum.run(&[r.clone()], &ctx()).len(), 1);
        r.event_type = Some("TRAD".into());
        assert!(SftrEventTypeEnum.run(&[r], &ctx()).is_empty());
    }

    #[test]
    fn sftr_master_agreement_type_enum_flags_and_accepts() {
        let mut r = SftrRecord {
            master_agreement_type: Some("XYZ".into()),
            ..Default::default()
        };
        assert_eq!(
            SftrMasterAgreementTypeEnum.run(&[r.clone()], &ctx()).len(),
            1
        );
        r.master_agreement_type = Some("GMRA".into());
        assert!(SftrMasterAgreementTypeEnum.run(&[r], &ctx()).is_empty());
    }

    #[test]
    fn sftr_lending_fee_negative_flags_and_accepts() {
        let r = SftrRecord {
            lending_fee: Some(Decimal::from(-1)),
            ..Default::default()
        };
        assert_eq!(SftrLendingFeeNegative.run(&[r], &ctx()).len(), 1);
        let r = SftrRecord {
            lending_fee: Some(Decimal::from(10)),
            ..Default::default()
        };
        assert!(SftrLendingFeeNegative.run(&[r], &ctx()).is_empty());
    }
}
