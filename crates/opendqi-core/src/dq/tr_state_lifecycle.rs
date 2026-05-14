//! Cross-batch lifecycle checks for the EMIR TSR (`auth.107`). Consume
//! a `prior` slice loaded from the SQLite history store and surface
//! drift between two consecutive snapshots: dropped UTIs, valuation
//! regression, maturity changed, collateral portfolio reassigned.

use std::collections::HashMap;

use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, Regime, Severity, TrStateRecord};

/// A cross-batch EMIR TSR lifecycle check.
pub trait TrStateLifecycleCheck: Send + Sync {
    /// Stable identifier, e.g. `EMIR.TST.LFC.UTI_DROPPED_WITHOUT_TERMINATION`.
    fn id(&self) -> &'static str;
    /// The DQ dimension this check belongs to.
    fn dimension(&self) -> DqDimension;
    /// Default severity for issues raised by this check.
    fn severity(&self) -> Severity;
    /// Execute the check.
    fn run(
        &self,
        current: &[TrStateRecord],
        prior: &[TrStateRecord],
        ctx: &CheckContext,
    ) -> Vec<DqIssue>;
}

fn is_outstanding(r: &TrStateRecord) -> bool {
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
        regime: Regime::Emir,
        severity,
        dimension,
        record_id,
        uti: Some(uti.to_owned()),
        field: Some(field.into()),
        value,
        message,
        source_file,
        evidence: Vec::new(),
    }
}

/// `EMIR.TST.LFC.UTI_DROPPED_WITHOUT_TERMINATION` — a UTI outstanding
/// in the prior snapshot is absent from the current one.
pub struct EmirTstLfcUtiDroppedWithoutTermination;

impl TrStateLifecycleCheck for EmirTstLfcUtiDroppedWithoutTermination {
    fn id(&self) -> &'static str {
        "EMIR.TST.LFC.UTI_DROPPED_WITHOUT_TERMINATION"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(
        &self,
        current: &[TrStateRecord],
        prior: &[TrStateRecord],
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
                        "UTI {uti} was outstanding in the prior TSR but is absent from the current snapshot — verify a TERM lifecycle event was reported."
                    ),
                    p.source_file.clone(),
                ));
            }
        }
        out
    }
}

/// `EMIR.TST.LFC.VALUATION_REGRESSION` — current `valuation_amount`
/// drops by > 50% compared to the prior snapshot's amount.
pub struct EmirTstLfcValuationRegression;

const VALUATION_REGRESSION_PCT: f64 = 0.50;

impl TrStateLifecycleCheck for EmirTstLfcValuationRegression {
    fn id(&self) -> &'static str {
        "EMIR.TST.LFC.VALUATION_REGRESSION"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(
        &self,
        current: &[TrStateRecord],
        prior: &[TrStateRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        let prior_by_uti: HashMap<&str, &TrStateRecord> = prior
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
            let (Some(prev), Some(curr)) = (p.valuation_amount, c.valuation_amount) else {
                continue;
            };
            let pf = prev.to_string().parse::<f64>().unwrap_or(f64::NAN);
            let cf = curr.to_string().parse::<f64>().unwrap_or(f64::NAN);
            if !pf.is_finite() || !cf.is_finite() || pf.abs() < f64::EPSILON {
                continue;
            }
            if cf < pf {
                let drop = (pf - cf).abs() / pf.abs();
                if drop > VALUATION_REGRESSION_PCT {
                    out.push(issue(
                        self.id(),
                        self.severity(),
                        self.dimension(),
                        uti,
                        c.record_id.clone(),
                        "valuation_amount",
                        Some(format!("prev={prev} curr={curr}")),
                        format!(
                            "valuation_amount on UTI {uti} dropped {:.0}% (from {prev} to {curr}).",
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

/// `EMIR.TST.LFC.MATURITY_CHANGED` — same UTI's `maturity_date`
/// differs between two snapshots.
pub struct EmirTstLfcMaturityChanged;

impl TrStateLifecycleCheck for EmirTstLfcMaturityChanged {
    fn id(&self) -> &'static str {
        "EMIR.TST.LFC.MATURITY_CHANGED"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(
        &self,
        current: &[TrStateRecord],
        prior: &[TrStateRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        let prior_by_uti: HashMap<&str, &TrStateRecord> = prior
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
            if let (Some(prev), Some(curr)) = (p.maturity_date, c.maturity_date) {
                if prev != curr {
                    out.push(issue(
                        self.id(),
                        self.severity(),
                        self.dimension(),
                        uti,
                        c.record_id.clone(),
                        "maturity_date",
                        Some(format!("prev={prev} curr={curr}")),
                        format!("maturity_date on UTI {uti} changed from {prev} to {curr}."),
                        c.source_file.clone(),
                    ));
                }
            }
        }
        out
    }
}

/// `EMIR.TST.LFC.COLLATERAL_PORTFOLIO_CHANGED` — same UTI's
/// `collateral_portfolio_code` differs between two snapshots.
pub struct EmirTstLfcCollateralPortfolioChanged;

impl TrStateLifecycleCheck for EmirTstLfcCollateralPortfolioChanged {
    fn id(&self) -> &'static str {
        "EMIR.TST.LFC.COLLATERAL_PORTFOLIO_CHANGED"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(
        &self,
        current: &[TrStateRecord],
        prior: &[TrStateRecord],
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        let prior_by_uti: HashMap<&str, &TrStateRecord> = prior
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
                p.collateral_portfolio_code.as_deref(),
                c.collateral_portfolio_code.as_deref(),
            ) {
                if !prev.eq_ignore_ascii_case(curr) {
                    out.push(issue(
                        self.id(),
                        self.severity(),
                        self.dimension(),
                        uti,
                        c.record_id.clone(),
                        "collateral_portfolio_code",
                        Some(format!("prev={prev} curr={curr}")),
                        format!(
                            "collateral_portfolio_code on UTI {uti} changed from {prev} to {curr}."
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
    use chrono::NaiveDate;
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

    fn rec(uti: &str) -> TrStateRecord {
        TrStateRecord {
            uti: Some(uti.into()),
            status: Some("OUTSTANDING".into()),
            ..Default::default()
        }
    }

    #[test]
    fn uti_dropped_flags_and_accepts() {
        let prior = vec![rec("U1"), rec("U2")];
        let current = vec![rec("U1")];
        let out = EmirTstLfcUtiDroppedWithoutTermination.run(&current, &prior, &ctx());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].uti.as_deref(), Some("U2"));
        let out2 = EmirTstLfcUtiDroppedWithoutTermination.run(&prior, &prior, &ctx());
        assert!(out2.is_empty());
    }

    #[test]
    fn uti_dropped_skips_terminated_prior() {
        let mut p = rec("U-TERM");
        p.status = Some("TERMINATED".into());
        let out = EmirTstLfcUtiDroppedWithoutTermination.run(&[], &[p], &ctx());
        assert!(out.is_empty());
    }

    #[test]
    fn valuation_regression_flags_and_accepts() {
        let mut p = rec("U");
        p.valuation_amount = Some(Decimal::from(1000));
        let mut c = rec("U");
        c.valuation_amount = Some(Decimal::from(100));
        assert_eq!(
            EmirTstLfcValuationRegression
                .run(&[c], &[p.clone()], &ctx())
                .len(),
            1
        );
        let mut c2 = rec("U");
        c2.valuation_amount = Some(Decimal::from(950));
        assert!(EmirTstLfcValuationRegression
            .run(&[c2], &[p], &ctx())
            .is_empty());
    }

    #[test]
    fn maturity_changed_flags_and_accepts() {
        let mut p = rec("U");
        p.maturity_date = NaiveDate::from_ymd_opt(2030, 1, 1);
        let mut c = rec("U");
        c.maturity_date = NaiveDate::from_ymd_opt(2031, 1, 1);
        assert_eq!(
            EmirTstLfcMaturityChanged
                .run(&[c], &[p.clone()], &ctx())
                .len(),
            1
        );
        let mut c2 = rec("U");
        c2.maturity_date = p.maturity_date;
        assert!(EmirTstLfcMaturityChanged
            .run(&[c2], &[p], &ctx())
            .is_empty());
    }

    #[test]
    fn portfolio_changed_flags_and_accepts() {
        let mut p = rec("U");
        p.collateral_portfolio_code = Some("PORT-A".into());
        let mut c = rec("U");
        c.collateral_portfolio_code = Some("PORT-B".into());
        assert_eq!(
            EmirTstLfcCollateralPortfolioChanged
                .run(&[c], &[p.clone()], &ctx())
                .len(),
            1
        );
        let mut c2 = rec("U");
        c2.collateral_portfolio_code = Some("port-a".into()); // case-insensitive
        assert!(EmirTstLfcCollateralPortfolioChanged
            .run(&[c2], &[p], &ctx())
            .is_empty());
    }
}
