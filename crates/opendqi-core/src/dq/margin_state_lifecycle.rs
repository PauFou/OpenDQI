//! Cross-batch lifecycle checks for the EMIR MSR (`auth.109`).
//! Operate on pairs of `MarginStateRecord` snapshots: current batch
//! vs. prior batch loaded from the history store.

use std::collections::HashMap;

use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, MarginStateRecord, Regime, Severity};

/// A cross-batch EMIR MSR lifecycle check.
pub trait MarginStateLifecycleCheck: Send + Sync {
    /// Stable identifier.
    fn id(&self) -> &'static str;
    /// DQ dimension.
    fn dimension(&self) -> DqDimension;
    /// Default severity.
    fn severity(&self) -> Severity;
    /// Execute.
    fn run(
        &self,
        current: &[MarginStateRecord],
        prior: &[MarginStateRecord],
        ctx: &CheckContext,
    ) -> Vec<DqIssue>;
}

#[allow(clippy::too_many_arguments)]
fn issue(
    check_id: &str,
    severity: Severity,
    dimension: DqDimension,
    uti: Option<&str>,
    record_id: Option<String>,
    field: &str,
    value: Option<String>,
    message: String,
    source_file: Option<String>,
) -> DqIssue {
    DqIssue {
        check_id: check_id.into(),
        regime: Regime::Emir,
        severity,
        dimension,
        record_id,
        uti: uti.map(String::from),
        field: Some(field.into()),
        value,
        message,
        source_file,
    }
}

/// `EMIR.MSR.LFC.COLLATERAL_MARKET_VALUE_REGRESSION` — same UTI's
/// `collateral_market_value` drops by > 30% between snapshots.
pub struct EmirMsrLfcCollateralMarketValueRegression;

const CMV_REGRESSION_PCT: f64 = 0.30;

impl MarginStateLifecycleCheck for EmirMsrLfcCollateralMarketValueRegression {
    fn id(&self) -> &'static str {
        "EMIR.MSR.LFC.COLLATERAL_MARKET_VALUE_REGRESSION"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(
        &self,
        current: &[MarginStateRecord],
        prior: &[MarginStateRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        let prior_by_uti: HashMap<&str, &MarginStateRecord> = prior
            .iter()
            .filter_map(|r| r.uti.as_deref().map(|u| (u.trim(), r)))
            .filter(|(u, _)| !u.is_empty())
            .collect();
        let mut out = Vec::new();
        for c in current {
            let Some(uti) = c.uti.as_deref().map(str::trim).filter(|u| !u.is_empty()) else {
                continue;
            };
            let Some(p) = prior_by_uti.get(uti) else {
                continue;
            };
            let (Some(prev), Some(curr)) = (p.collateral_market_value, c.collateral_market_value)
            else {
                continue;
            };
            let pf = prev.to_string().parse::<f64>().unwrap_or(f64::NAN);
            let cf = curr.to_string().parse::<f64>().unwrap_or(f64::NAN);
            if !pf.is_finite() || !cf.is_finite() || pf.abs() < f64::EPSILON {
                continue;
            }
            if cf < pf {
                let drop = (pf - cf).abs() / pf.abs();
                if drop > CMV_REGRESSION_PCT {
                    out.push(issue(
                        self.id(),
                        self.severity(),
                        self.dimension(),
                        Some(uti),
                        c.record_id.clone(),
                        "collateral_market_value",
                        Some(format!("prev={prev} curr={curr}")),
                        format!(
                            "collateral_market_value on UTI {uti} dropped {:.0}% (from {prev} to {curr}).",
                            drop * 100.0
                        ),
                        c.source_file.clone(),
                    ));
                }
            }
        }
        out
    }
}

/// `EMIR.MSR.LFC.CATEGORY_CHANGED` — same UTI's
/// `collateralization_category` differs between snapshots.
pub struct EmirMsrLfcCategoryChanged;

impl MarginStateLifecycleCheck for EmirMsrLfcCategoryChanged {
    fn id(&self) -> &'static str {
        "EMIR.MSR.LFC.CATEGORY_CHANGED"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(
        &self,
        current: &[MarginStateRecord],
        prior: &[MarginStateRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        let prior_by_uti: HashMap<&str, &MarginStateRecord> = prior
            .iter()
            .filter_map(|r| r.uti.as_deref().map(|u| (u.trim(), r)))
            .filter(|(u, _)| !u.is_empty())
            .collect();
        let mut out = Vec::new();
        for c in current {
            let Some(uti) = c.uti.as_deref().map(str::trim).filter(|u| !u.is_empty()) else {
                continue;
            };
            let Some(p) = prior_by_uti.get(uti) else {
                continue;
            };
            if let (Some(prev), Some(curr)) = (
                p.collateralization_category.as_deref(),
                c.collateralization_category.as_deref(),
            ) {
                if !prev.eq_ignore_ascii_case(curr) {
                    out.push(issue(
                        self.id(),
                        self.severity(),
                        self.dimension(),
                        Some(uti),
                        c.record_id.clone(),
                        "collateralization_category",
                        Some(format!("prev={prev} curr={curr}")),
                        format!(
                            "collateralization_category on UTI {uti} changed from {prev} to {curr}."
                        ),
                        c.source_file.clone(),
                    ));
                }
            }
        }
        out
    }
}

/// `EMIR.MSR.LFC.HAIRCUT_DRIFT` — same UTI's `haircut_applied`
/// changes by > 50% (relative) between snapshots.
pub struct EmirMsrLfcHaircutDrift;

const HAIRCUT_DRIFT_PCT: f64 = 0.50;

impl MarginStateLifecycleCheck for EmirMsrLfcHaircutDrift {
    fn id(&self) -> &'static str {
        "EMIR.MSR.LFC.HAIRCUT_DRIFT"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(
        &self,
        current: &[MarginStateRecord],
        prior: &[MarginStateRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        let prior_by_uti: HashMap<&str, &MarginStateRecord> = prior
            .iter()
            .filter_map(|r| r.uti.as_deref().map(|u| (u.trim(), r)))
            .filter(|(u, _)| !u.is_empty())
            .collect();
        let mut out = Vec::new();
        for c in current {
            let Some(uti) = c.uti.as_deref().map(str::trim).filter(|u| !u.is_empty()) else {
                continue;
            };
            let Some(p) = prior_by_uti.get(uti) else {
                continue;
            };
            let (Some(prev), Some(curr)) = (p.haircut_applied, c.haircut_applied) else {
                continue;
            };
            let pf = prev.to_string().parse::<f64>().unwrap_or(f64::NAN);
            let cf = curr.to_string().parse::<f64>().unwrap_or(f64::NAN);
            if !pf.is_finite() || !cf.is_finite() || pf.abs() < f64::EPSILON {
                continue;
            }
            let drift = (cf - pf).abs() / pf.abs();
            if drift > HAIRCUT_DRIFT_PCT {
                out.push(issue(
                    self.id(),
                    self.severity(),
                    self.dimension(),
                    Some(uti),
                    c.record_id.clone(),
                    "haircut_applied",
                    Some(format!("prev={prev} curr={curr}")),
                    format!(
                        "haircut_applied on UTI {uti} drifted {:.0}% (from {prev} to {curr}).",
                        drift * 100.0
                    ),
                    c.source_file.clone(),
                ));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    fn ctx() -> CheckContext {
        CheckContext {
            thresholds: Default::default(),
            today: chrono::NaiveDate::from_ymd_opt(2026, 5, 14).unwrap(),
            now: chrono::DateTime::parse_from_rfc3339("2026-05-14T08:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
        }
    }

    fn rec(uti: &str) -> MarginStateRecord {
        MarginStateRecord {
            uti: Some(uti.into()),
            ..Default::default()
        }
    }

    #[test]
    fn cmv_regression_flags_and_accepts() {
        let mut p = rec("U");
        p.collateral_market_value = Some(Decimal::from(1000));
        let mut c = rec("U");
        c.collateral_market_value = Some(Decimal::from(500));
        assert_eq!(
            EmirMsrLfcCollateralMarketValueRegression
                .run(&[c], &[p.clone()], &ctx())
                .len(),
            1
        );
        let mut c2 = rec("U");
        c2.collateral_market_value = Some(Decimal::from(900));
        assert!(EmirMsrLfcCollateralMarketValueRegression
            .run(&[c2], &[p], &ctx())
            .is_empty());
    }

    #[test]
    fn category_changed_flags_and_accepts() {
        let mut p = rec("U");
        p.collateralization_category = Some("FCOL".into());
        let mut c = rec("U");
        c.collateralization_category = Some("PCOL".into());
        assert_eq!(
            EmirMsrLfcCategoryChanged
                .run(&[c], &[p.clone()], &ctx())
                .len(),
            1
        );
        let mut c2 = rec("U");
        c2.collateralization_category = Some("fcol".into());
        assert!(EmirMsrLfcCategoryChanged
            .run(&[c2], &[p], &ctx())
            .is_empty());
    }

    #[test]
    fn haircut_drift_flags_and_accepts() {
        let mut p = rec("U");
        p.haircut_applied = Some(Decimal::new(5, 2));
        let mut c = rec("U");
        c.haircut_applied = Some(Decimal::new(20, 2)); // 0.05 → 0.20, drift = 300%
        assert_eq!(
            EmirMsrLfcHaircutDrift.run(&[c], &[p.clone()], &ctx()).len(),
            1
        );
        let mut c2 = rec("U");
        c2.haircut_applied = Some(Decimal::new(6, 2)); // drift = 20%
        assert!(EmirMsrLfcHaircutDrift.run(&[c2], &[p], &ctx()).is_empty());
    }
}
