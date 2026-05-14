//! Cross-batch lifecycle checks for the SFTR TSR (`auth.079`). Mirror
//! of the EMIR TSR lifecycle layer on `SftrTrStateRecord`. Three
//! checks: UTI dropped without termination, collateral value
//! regression, haircut changed.

use std::collections::HashMap;

use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrTrStateRecord};

/// A cross-batch SFTR TSR lifecycle check.
pub trait SftrTrStateLifecycleCheck: Send + Sync {
    /// Stable identifier.
    fn id(&self) -> &'static str;
    /// DQ dimension.
    fn dimension(&self) -> DqDimension;
    /// Default severity.
    fn severity(&self) -> Severity;
    /// Execute.
    fn run(
        &self,
        current: &[SftrTrStateRecord],
        prior: &[SftrTrStateRecord],
        ctx: &CheckContext,
    ) -> Vec<DqIssue>;
}

fn is_outstanding(r: &SftrTrStateRecord) -> bool {
    match r.status.as_deref() {
        None => true,
        Some(s) => {
            let s = s.trim();
            s.is_empty()
                || s.eq_ignore_ascii_case("OUTSTANDING")
                || s.eq_ignore_ascii_case("ACTIVE")
                || s.eq_ignore_ascii_case("LIVE")
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn issue(
    check_id: &str,
    severity: Severity,
    dimension: DqDimension,
    uti: &str,
    record_id: Option<String>,
    field: &str,
    value: Option<String>,
    message: String,
    source_file: Option<String>,
) -> DqIssue {
    DqIssue {
        check_id: check_id.into(),
        regime: Regime::Sftr,
        severity,
        dimension,
        record_id,
        uti: Some(uti.to_owned()),
        field: Some(field.into()),
        value,
        message,
        source_file,
    }
}

/// `SFTR.TST.LFC.UTI_DROPPED_WITHOUT_TERMINATION`.
pub struct SftrTstLfcUtiDroppedWithoutTermination;

impl SftrTrStateLifecycleCheck for SftrTstLfcUtiDroppedWithoutTermination {
    fn id(&self) -> &'static str {
        "SFTR.TST.LFC.UTI_DROPPED_WITHOUT_TERMINATION"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(
        &self,
        current: &[SftrTrStateRecord],
        prior: &[SftrTrStateRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        let current_utis: std::collections::HashSet<&str> = current
            .iter()
            .filter_map(|r| r.uti.as_deref())
            .map(str::trim)
            .filter(|u| !u.is_empty())
            .collect();
        let mut out = Vec::new();
        for p in prior.iter().filter(|r| is_outstanding(r)) {
            let Some(uti) = p.uti.as_deref().map(str::trim).filter(|u| !u.is_empty()) else {
                continue;
            };
            if !current_utis.contains(uti) {
                out.push(issue(
                    self.id(),
                    self.severity(),
                    self.dimension(),
                    uti,
                    p.record_id.clone(),
                    "uti",
                    None,
                    format!(
                        "SFT UTI {uti} was outstanding in the prior SFTR TSR but is absent from the current snapshot."
                    ),
                    p.source_file.clone(),
                ));
            }
        }
        out
    }
}

/// `SFTR.TST.LFC.COLLATERAL_VALUE_REGRESSION`.
pub struct SftrTstLfcCollateralValueRegression;

const COLLATERAL_REGRESSION_PCT: f64 = 0.50;

impl SftrTrStateLifecycleCheck for SftrTstLfcCollateralValueRegression {
    fn id(&self) -> &'static str {
        "SFTR.TST.LFC.COLLATERAL_VALUE_REGRESSION"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(
        &self,
        current: &[SftrTrStateRecord],
        prior: &[SftrTrStateRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        let prior_by_uti: HashMap<&str, &SftrTrStateRecord> = prior
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
            let (Some(prev), Some(curr)) = (p.collateral_value, c.collateral_value) else {
                continue;
            };
            let pf = prev.to_string().parse::<f64>().unwrap_or(f64::NAN);
            let cf = curr.to_string().parse::<f64>().unwrap_or(f64::NAN);
            if !pf.is_finite() || !cf.is_finite() || pf.abs() < f64::EPSILON {
                continue;
            }
            if cf < pf {
                let drop = (pf - cf).abs() / pf.abs();
                if drop > COLLATERAL_REGRESSION_PCT {
                    out.push(issue(
                        self.id(),
                        self.severity(),
                        self.dimension(),
                        uti,
                        c.record_id.clone(),
                        "collateral_value",
                        Some(format!("prev={prev} curr={curr}")),
                        format!(
                            "collateral_value on SFT UTI {uti} dropped {:.0}% (from {prev} to {curr}).",
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

/// `SFTR.TST.LFC.HAIRCUT_CHANGED`.
pub struct SftrTstLfcHaircutChanged;

impl SftrTrStateLifecycleCheck for SftrTstLfcHaircutChanged {
    fn id(&self) -> &'static str {
        "SFTR.TST.LFC.HAIRCUT_CHANGED"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(
        &self,
        current: &[SftrTrStateRecord],
        prior: &[SftrTrStateRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        let prior_by_uti: HashMap<&str, &SftrTrStateRecord> = prior
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
            if let (Some(prev), Some(curr)) = (p.haircut, c.haircut) {
                if prev != curr {
                    out.push(issue(
                        self.id(),
                        self.severity(),
                        self.dimension(),
                        uti,
                        c.record_id.clone(),
                        "haircut",
                        Some(format!("prev={prev} curr={curr}")),
                        format!(
                            "haircut on SFT UTI {uti} changed from {prev} to {curr} — verify collateral re-pricing."
                        ),
                        c.source_file.clone(),
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

    fn rec(uti: &str) -> SftrTrStateRecord {
        SftrTrStateRecord {
            uti: Some(uti.into()),
            status: Some("OUTSTANDING".into()),
            ..Default::default()
        }
    }

    #[test]
    fn uti_dropped_flags_and_accepts() {
        let prior = vec![rec("U1"), rec("U2")];
        let current = vec![rec("U1")];
        let out = SftrTstLfcUtiDroppedWithoutTermination.run(&current, &prior, &ctx());
        assert_eq!(out.len(), 1);
        let same = SftrTstLfcUtiDroppedWithoutTermination.run(&prior, &prior, &ctx());
        assert!(same.is_empty());
    }

    #[test]
    fn collateral_value_regression_flags_and_accepts() {
        let mut p = rec("U");
        p.collateral_value = Some(Decimal::from(1000));
        let mut c = rec("U");
        c.collateral_value = Some(Decimal::from(100));
        assert_eq!(
            SftrTstLfcCollateralValueRegression
                .run(&[c], &[p.clone()], &ctx())
                .len(),
            1
        );
        let mut c2 = rec("U");
        c2.collateral_value = Some(Decimal::from(950));
        assert!(SftrTstLfcCollateralValueRegression
            .run(&[c2], &[p], &ctx())
            .is_empty());
    }

    #[test]
    fn haircut_changed_flags_and_accepts() {
        let mut p = rec("U");
        p.haircut = Some(Decimal::new(5, 2));
        let mut c = rec("U");
        c.haircut = Some(Decimal::new(10, 2));
        assert_eq!(
            SftrTstLfcHaircutChanged
                .run(&[c], &[p.clone()], &ctx())
                .len(),
            1
        );
        let mut c2 = rec("U");
        c2.haircut = p.haircut;
        assert!(SftrTstLfcHaircutChanged.run(&[c2], &[p], &ctx()).is_empty());
    }
}
