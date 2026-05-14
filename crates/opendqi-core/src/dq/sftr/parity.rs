//! SFTR parity push — checks ported from the EMIR catalog that
//! consume fields already on `SftrRecord`. See
//! [`docs/emir-vs-sftr-parity.md`] for the comparative audit.
//!
//! Nine checks: ABNORMAL_MATURITY, LOAN_ABNORMAL_MAGNITUDE,
//! EVENT_BEFORE_EXECUTION, REPORTING_BEFORE_EXECUTION,
//! MATURITY_IN_PAST, TERMINATION_AFTER_MATURITY, MODI_PRESERVES_UTI,
//! ACTION_EVENT_COMPATIBILITY, LATE_REPORTING_SETTLEMENT.

use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use rust_decimal::Decimal;

use crate::dq::{CheckContext, SftrCheck};
use crate::model::{DqDimension, DqIssue, Regime, Severity, SftrRecord};

fn issue(
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

fn ts_strictly_before(a: DateTime<Utc>, b: DateTime<Utc>) -> bool {
    a < b
}

// -------- SFTR.ACC.ABNORMAL_MATURITY -----------------------------

/// Check implementation. Mirrors `EMIR.ACC.ABNORMAL_MATURITY` :
/// placeholder dates + far-future ceiling. `maturity_date <
/// effective_date` is already covered by `SFTR.CON.MATURITY_BEFORE_EFFECTIVE`,
/// so we don't duplicate that arm here.
pub struct SftrAbnormalMaturity;

impl SftrCheck for SftrAbnormalMaturity {
    fn id(&self) -> &'static str {
        "SFTR.ACC.ABNORMAL_MATURITY"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[SftrRecord], ctx: &CheckContext) -> Vec<DqIssue> {
        let placeholders: &[NaiveDate] = &ctx.thresholds.maturity.placeholder_dates;
        let abnormal_years = ctx.thresholds.maturity.abnormal_maturity_years;
        let max_normal = ctx
            .today
            .with_year(ctx.today.year() + abnormal_years)
            .unwrap_or(ctx.today);
        let mut out = Vec::new();
        for r in records {
            let Some(maturity) = r.maturity_date else {
                continue;
            };
            let reason = if placeholders.contains(&maturity) {
                Some(format!("Maturity {maturity} is a placeholder date."))
            } else if maturity > max_normal {
                Some(format!(
                    "Maturity {maturity} is more than {abnormal_years} years after {today}.",
                    today = ctx.today
                ))
            } else {
                None
            };
            if let Some(message) = reason {
                out.push(issue(
                    self.id(),
                    self.severity(),
                    self.dimension(),
                    r,
                    "maturity_date",
                    Some(maturity.to_string()),
                    message,
                ));
            }
        }
        out
    }
}

// -------- SFTR.ACC.LOAN_ABNORMAL_MAGNITUDE -----------------------

/// Check implementation. Same 10^15 sanity ceiling as
/// `EMIR.ACC.NOTIONAL_ABNORMAL_MAGNITUDE` — flags loan values that
/// dwarf any plausible SFT principal and likely stem from a unit /
/// scale bug at the source.
pub struct SftrLoanAbnormalMagnitude;

const LOAN_ABNORMAL_CEILING: i64 = 1_000_000_000_000_000; // 10^15

impl SftrCheck for SftrLoanAbnormalMagnitude {
    fn id(&self) -> &'static str {
        "SFTR.ACC.LOAN_ABNORMAL_MAGNITUDE"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Accuracy
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[SftrRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        let ceiling = Decimal::from(LOAN_ABNORMAL_CEILING);
        let mut out = Vec::new();
        for r in records {
            if let Some(loan) = r.loan_value {
                if loan.abs() > ceiling {
                    out.push(issue(
                        self.id(),
                        self.severity(),
                        self.dimension(),
                        r,
                        "loan_value",
                        Some(loan.to_string()),
                        format!(
                            "loan_value {loan} exceeds the {LOAN_ABNORMAL_CEILING} EUR sanity ceiling — verify the unit / scale."
                        ),
                    ));
                }
            }
        }
        out
    }
}

// -------- SFTR.CON.EVENT_BEFORE_EXECUTION ------------------------

/// Check implementation. `event_timestamp` cannot precede the trade's
/// `execution_timestamp` — events happen at or after execution.
pub struct SftrEventBeforeExecution;

impl SftrCheck for SftrEventBeforeExecution {
    fn id(&self) -> &'static str {
        "SFTR.CON.EVENT_BEFORE_EXECUTION"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[SftrRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        let mut out = Vec::new();
        for r in records {
            let (Some(event), Some(exec)) = (r.event_timestamp, r.execution_timestamp) else {
                continue;
            };
            if ts_strictly_before(event, exec) {
                out.push(issue(
                    self.id(),
                    self.severity(),
                    self.dimension(),
                    r,
                    "event_timestamp",
                    Some(event.to_rfc3339()),
                    format!("event_timestamp {event} precedes execution_timestamp {exec}."),
                ));
            }
        }
        out
    }
}

// -------- SFTR.CON.REPORTING_BEFORE_EXECUTION --------------------

/// Check implementation. `reporting_timestamp` precedes
/// `execution_timestamp` — physically impossible.
pub struct SftrReportingBeforeExecution;

impl SftrCheck for SftrReportingBeforeExecution {
    fn id(&self) -> &'static str {
        "SFTR.CON.REPORTING_BEFORE_EXECUTION"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[SftrRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        let mut out = Vec::new();
        for r in records {
            let (Some(rep), Some(exec)) = (r.reporting_timestamp, r.execution_timestamp) else {
                continue;
            };
            if ts_strictly_before(rep, exec) {
                out.push(issue(
                    self.id(),
                    self.severity(),
                    self.dimension(),
                    r,
                    "reporting_timestamp",
                    Some(rep.to_rfc3339()),
                    format!("reporting_timestamp {rep} precedes execution_timestamp {exec}."),
                ));
            }
        }
        out
    }
}

// -------- SFTR.CON.MATURITY_IN_PAST ------------------------------

/// Check implementation. SFT not yet terminated but
/// `maturity_date < today` — the contract has matured without a
/// termination event. Complements `SFTR.CON.MATURITY_BEFORE_EFFECTIVE`
/// which checks the opposite ordering.
pub struct SftrMaturityInPast;

impl SftrCheck for SftrMaturityInPast {
    fn id(&self) -> &'static str {
        "SFTR.CON.MATURITY_IN_PAST"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[SftrRecord], ctx: &CheckContext) -> Vec<DqIssue> {
        let mut out = Vec::new();
        for r in records {
            if r.termination_date.is_some() {
                continue;
            }
            let Some(maturity) = r.maturity_date else {
                continue;
            };
            if maturity < ctx.today {
                out.push(issue(
                    self.id(),
                    self.severity(),
                    self.dimension(),
                    r,
                    "maturity_date",
                    Some(maturity.to_string()),
                    format!(
                        "maturity_date {maturity} is in the past but no termination_date is recorded (today={today}).",
                        today = ctx.today
                    ),
                ));
            }
        }
        out
    }
}

// -------- SFTR.CON.TERMINATION_AFTER_MATURITY --------------------

/// Check implementation. A termination posted strictly after
/// maturity is suspicious — once an SFT matures it should be reported
/// as matured, not terminated.
pub struct SftrTerminationAfterMaturity;

impl SftrCheck for SftrTerminationAfterMaturity {
    fn id(&self) -> &'static str {
        "SFTR.CON.TERMINATION_AFTER_MATURITY"
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
            let (Some(termination), Some(maturity)) = (r.termination_date, r.maturity_date) else {
                continue;
            };
            if termination > maturity {
                out.push(issue(
                    self.id(),
                    self.severity(),
                    self.dimension(),
                    r,
                    "termination_date",
                    Some(termination.to_string()),
                    format!(
                        "termination_date {termination} is strictly after maturity_date {maturity}."
                    ),
                ));
            }
        }
        out
    }
}

// -------- SFTR.CON.MODI_PRESERVES_UTI ----------------------------

/// Check implementation. A MODI action keeps the same UTI; the
/// `prior_uti` field, when set, must equal `uti`. Mirrors
/// `EMIR.CON.MODI_PRESERVES_UTI`.
pub struct SftrModiPreservesUti;

impl SftrCheck for SftrModiPreservesUti {
    fn id(&self) -> &'static str {
        "SFTR.CON.MODI_PRESERVES_UTI"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[SftrRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        let mut out = Vec::new();
        for r in records {
            let is_modi = r
                .action_type
                .as_deref()
                .map(|a| a.trim().eq_ignore_ascii_case("MODI"))
                .unwrap_or(false);
            if !is_modi {
                continue;
            }
            let Some(prior) = r.prior_uti.as_deref().map(str::trim) else {
                continue;
            };
            if prior.is_empty() {
                continue;
            }
            let current = r.uti.as_deref().map(str::trim).unwrap_or("");
            if !prior.eq_ignore_ascii_case(current) {
                out.push(issue(
                    self.id(),
                    self.severity(),
                    self.dimension(),
                    r,
                    "prior_uti",
                    Some(prior.to_owned()),
                    format!(
                        "MODI action must preserve the UTI: prior_uti={prior} ≠ uti={current}."
                    ),
                ));
            }
        }
        out
    }
}

// -------- SFTR.CON.ACTION_EVENT_COMPATIBILITY --------------------

/// Check implementation. `action_type` and `event_type` must align
/// for the well-known SFTR pairs: NEWT/TRAD, MODI/MODI, ETRM/ETRM,
/// COLU/COLU, REUU/REUU, CORR/CORR, MARU/MARU. Skips records where
/// either field is missing or where the action is not in the
/// matrix above (lets specialised enums own those cases).
pub struct SftrActionEventCompatibility;

impl SftrCheck for SftrActionEventCompatibility {
    fn id(&self) -> &'static str {
        "SFTR.CON.ACTION_EVENT_COMPATIBILITY"
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
            let (Some(action), Some(event)) = (r.action_type.as_deref(), r.event_type.as_deref())
            else {
                continue;
            };
            let action = action.trim().to_uppercase();
            let event = event.trim().to_uppercase();
            if action.is_empty() || event.is_empty() {
                continue;
            }
            let expected: Option<&str> = match action.as_str() {
                "NEWT" => Some("TRAD"),
                "MODI" => Some("MODI"),
                "ETRM" => Some("ETRM"),
                "COLU" => Some("COLU"),
                "REUU" => Some("REUU"),
                "CORR" => Some("CORR"),
                "MARU" => Some("MARU"),
                _ => None,
            };
            let Some(expected) = expected else {
                continue;
            };
            if event != expected {
                out.push(issue(
                    self.id(),
                    self.severity(),
                    self.dimension(),
                    r,
                    "event_type",
                    Some(event.clone()),
                    format!(
                        "action_type {action} is typically paired with event_type {expected}, got {event}."
                    ),
                ));
            }
        }
        out
    }
}

// -------- SFTR.TIM.LATE_REPORTING_SETTLEMENT ---------------------

/// Check implementation. `settlement_date` is more than 7 days after
/// the `reporting_timestamp` — the SFT was reported well before its
/// expected settlement, which is suspicious post-trade.
pub struct SftrLateReportingSettlement;

const LATE_SETTLEMENT_THRESHOLD_DAYS: i64 = 7;

impl SftrCheck for SftrLateReportingSettlement {
    fn id(&self) -> &'static str {
        "SFTR.TIM.LATE_REPORTING_SETTLEMENT"
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Timeliness
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn run(&self, records: &[SftrRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        let max = Duration::days(LATE_SETTLEMENT_THRESHOLD_DAYS);
        let mut out = Vec::new();
        for r in records {
            let (Some(settlement), Some(reporting)) = (r.settlement_date, r.reporting_timestamp)
            else {
                continue;
            };
            let reporting_date = reporting.date_naive();
            let delta = settlement.signed_duration_since(reporting_date);
            if delta > max {
                out.push(issue(
                    self.id(),
                    self.severity(),
                    self.dimension(),
                    r,
                    "settlement_date",
                    Some(settlement.to_string()),
                    format!(
                        "settlement_date {settlement} is more than {LATE_SETTLEMENT_THRESHOLD_DAYS} days after reporting ({reporting_date}) — verify forward-settling SFT or reporting timing."
                    ),
                ));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, NaiveDate, Utc};

    fn ctx() -> CheckContext {
        let now = chrono::DateTime::parse_from_rfc3339("2026-05-14T08:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        CheckContext {
            thresholds: Default::default(),
            today: NaiveDate::from_ymd_opt(2026, 5, 14).unwrap(),
            now,
        }
    }

    fn ts(s: &str) -> Option<DateTime<Utc>> {
        Some(
            chrono::DateTime::parse_from_rfc3339(s)
                .unwrap()
                .with_timezone(&Utc),
        )
    }

    #[test]
    fn abnormal_maturity_placeholder_flags_and_normal_accepts() {
        let r = SftrRecord {
            maturity_date: NaiveDate::from_ymd_opt(2099, 12, 31),
            ..Default::default()
        };
        assert_eq!(SftrAbnormalMaturity.run(&[r], &ctx()).len(), 1);
        let r2 = SftrRecord {
            maturity_date: NaiveDate::from_ymd_opt(2030, 1, 1),
            ..Default::default()
        };
        assert!(SftrAbnormalMaturity.run(&[r2], &ctx()).is_empty());
    }

    #[test]
    fn abnormal_maturity_far_future_flags() {
        let r = SftrRecord {
            maturity_date: NaiveDate::from_ymd_opt(2200, 1, 1),
            ..Default::default()
        };
        assert_eq!(SftrAbnormalMaturity.run(&[r], &ctx()).len(), 1);
    }

    #[test]
    fn loan_abnormal_magnitude_flags_and_accepts() {
        let r = SftrRecord {
            loan_value: Some(Decimal::from(2_000_000_000_000_000i64)),
            ..Default::default()
        };
        assert_eq!(SftrLoanAbnormalMagnitude.run(&[r], &ctx()).len(), 1);
        let r2 = SftrRecord {
            loan_value: Some(Decimal::from(1_000_000_000i64)),
            ..Default::default()
        };
        assert!(SftrLoanAbnormalMagnitude.run(&[r2], &ctx()).is_empty());
    }

    #[test]
    fn event_before_execution_flags_and_accepts() {
        let r = SftrRecord {
            execution_timestamp: ts("2026-05-13T08:00:00Z"),
            event_timestamp: ts("2026-05-12T08:00:00Z"),
            ..Default::default()
        };
        assert_eq!(SftrEventBeforeExecution.run(&[r], &ctx()).len(), 1);
        let r2 = SftrRecord {
            execution_timestamp: ts("2026-05-13T08:00:00Z"),
            event_timestamp: ts("2026-05-13T09:00:00Z"),
            ..Default::default()
        };
        assert!(SftrEventBeforeExecution.run(&[r2], &ctx()).is_empty());
    }

    #[test]
    fn reporting_before_execution_flags_and_accepts() {
        let r = SftrRecord {
            execution_timestamp: ts("2026-05-13T08:00:00Z"),
            reporting_timestamp: ts("2026-05-13T07:00:00Z"),
            ..Default::default()
        };
        assert_eq!(SftrReportingBeforeExecution.run(&[r], &ctx()).len(), 1);
        let r2 = SftrRecord {
            execution_timestamp: ts("2026-05-13T08:00:00Z"),
            reporting_timestamp: ts("2026-05-13T18:00:00Z"),
            ..Default::default()
        };
        assert!(SftrReportingBeforeExecution.run(&[r2], &ctx()).is_empty());
    }

    #[test]
    fn maturity_in_past_flags_and_accepts() {
        let r = SftrRecord {
            maturity_date: NaiveDate::from_ymd_opt(2025, 1, 1),
            ..Default::default()
        };
        assert_eq!(SftrMaturityInPast.run(&[r], &ctx()).len(), 1);
        // terminated → skip
        let r2 = SftrRecord {
            maturity_date: NaiveDate::from_ymd_opt(2025, 1, 1),
            termination_date: NaiveDate::from_ymd_opt(2025, 1, 1),
            ..Default::default()
        };
        assert!(SftrMaturityInPast.run(&[r2], &ctx()).is_empty());
        // future maturity → accept
        let r3 = SftrRecord {
            maturity_date: NaiveDate::from_ymd_opt(2030, 1, 1),
            ..Default::default()
        };
        assert!(SftrMaturityInPast.run(&[r3], &ctx()).is_empty());
    }

    #[test]
    fn termination_after_maturity_flags_and_accepts() {
        let r = SftrRecord {
            maturity_date: NaiveDate::from_ymd_opt(2026, 6, 30),
            termination_date: NaiveDate::from_ymd_opt(2026, 7, 15),
            ..Default::default()
        };
        assert_eq!(SftrTerminationAfterMaturity.run(&[r], &ctx()).len(), 1);
        let r2 = SftrRecord {
            maturity_date: NaiveDate::from_ymd_opt(2026, 6, 30),
            termination_date: NaiveDate::from_ymd_opt(2026, 6, 1),
            ..Default::default()
        };
        assert!(SftrTerminationAfterMaturity.run(&[r2], &ctx()).is_empty());
    }

    #[test]
    fn modi_preserves_uti_flags_and_accepts() {
        let r = SftrRecord {
            action_type: Some("MODI".into()),
            uti: Some("U1".into()),
            prior_uti: Some("U2".into()),
            ..Default::default()
        };
        assert_eq!(SftrModiPreservesUti.run(&[r], &ctx()).len(), 1);
        let r2 = SftrRecord {
            action_type: Some("MODI".into()),
            uti: Some("U1".into()),
            prior_uti: Some("U1".into()),
            ..Default::default()
        };
        assert!(SftrModiPreservesUti.run(&[r2], &ctx()).is_empty());
        // non-MODI ignored
        let r3 = SftrRecord {
            action_type: Some("NEWT".into()),
            uti: Some("U1".into()),
            prior_uti: Some("U2".into()),
            ..Default::default()
        };
        assert!(SftrModiPreservesUti.run(&[r3], &ctx()).is_empty());
    }

    #[test]
    fn action_event_compatibility_flags_and_accepts() {
        let r = SftrRecord {
            action_type: Some("NEWT".into()),
            event_type: Some("MODI".into()),
            ..Default::default()
        };
        assert_eq!(SftrActionEventCompatibility.run(&[r], &ctx()).len(), 1);
        let r2 = SftrRecord {
            action_type: Some("ETRM".into()),
            event_type: Some("ETRM".into()),
            ..Default::default()
        };
        assert!(SftrActionEventCompatibility.run(&[r2], &ctx()).is_empty());
    }

    #[test]
    fn action_event_compatibility_skips_unknown_action() {
        let r = SftrRecord {
            action_type: Some("OTHR".into()),
            event_type: Some("WHATEVER".into()),
            ..Default::default()
        };
        assert!(SftrActionEventCompatibility.run(&[r], &ctx()).is_empty());
    }

    #[test]
    fn late_reporting_settlement_flags_and_accepts() {
        let r = SftrRecord {
            reporting_timestamp: ts("2026-05-13T08:00:00Z"),
            settlement_date: NaiveDate::from_ymd_opt(2026, 5, 25),
            ..Default::default()
        };
        assert_eq!(SftrLateReportingSettlement.run(&[r], &ctx()).len(), 1);
        let r2 = SftrRecord {
            reporting_timestamp: ts("2026-05-13T08:00:00Z"),
            settlement_date: NaiveDate::from_ymd_opt(2026, 5, 15),
            ..Default::default()
        };
        assert!(SftrLateReportingSettlement.run(&[r2], &ctx()).is_empty());
    }
}
