//! `EMIR.POS.UNDERLYING_ID_MISSING` — the position-set record
//! carries notional but no underlying instrument identifier.
//!
//! Per the auth.090 XSD, `UndrlygInstrm` is `[0..1]` so the
//! parser may report `underlying_id=None`. For a non-trivial
//! position (notional > 0) on a derivative contract, the
//! underlying ISIN should normally be present — its absence
//! breaks downstream cross-message linking against TR state
//! reports (auth.107) and signals incomplete reporting.
//!
//! Honest naming note: the v0.18 plan called this
//! `JURISDICTION_MISSING`, but auth.090 has no explicit
//! `jurisdiction` field at the XSD level — the closest
//! actionable completeness signal is `UndrlygInstrm/.../ISIN`
//! (the underlying identifier). The file name is kept for
//! continuity with the plan; the check_id and behaviour match
//! the actual XSD shape.

use rust_decimal::Decimal;

use super::EmirPosCheck;
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, EmirPositionSetRecord, Regime, Severity};

/// Check implementation.
pub struct EmirPosJurisdictionMissing;

const CHECK_ID: &str = "EMIR.POS.UNDERLYING_ID_MISSING";

impl EmirPosCheck for EmirPosJurisdictionMissing {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Completeness
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(&self, records: &[EmirPositionSetRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        let mut out = Vec::new();
        for r in records {
            // Only PosSet/CcyPosSet records carry the
            // underlying-instrument identifier in practice;
            // collateral kinds aggregate across underlyings.
            let kind = r.position_set_kind.as_deref().unwrap_or("");
            if kind != "PosSet" && kind != "CcyPosSet" {
                continue;
            }
            // Skip records without notional — the missing-
            // underlying signal is only meaningful when the
            // position is non-trivial.
            let Some(n) = r.notional else { continue };
            if n <= Decimal::ZERO {
                continue;
            }
            if r.underlying_id.is_none() {
                out.push(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Emir,
                    severity: Severity::High,
                    dimension: DqDimension::Completeness,
                    record_id: r.record_id.clone(),
                    uti: None,
                    field: Some("underlying_id".into()),
                    value: None,
                    message: format!(
                        "{kind} record reports notional {n} but no UndrlygInstrm/.../ISIN; \
                         breaks downstream cross-message linking against TR state reports"
                    ),
                    source_file: r.source_file.clone(),
                    evidence: Vec::new(),
                });
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
    fn fires_on_posset_with_notional_but_no_underlying() {
        let r = EmirPositionSetRecord {
            record_id: Some("R1".into()),
            position_set_kind: Some("PosSet".into()),
            notional: Some(Decimal::from(1000)),
            underlying_id: None,
            ..Default::default()
        };
        let out = EmirPosJurisdictionMissing.run(&[r], &ctx());
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn does_not_fire_when_underlying_present() {
        let r = EmirPositionSetRecord {
            position_set_kind: Some("PosSet".into()),
            notional: Some(Decimal::from(1000)),
            underlying_id: Some("DE000A1B2C34".into()),
            ..Default::default()
        };
        let out = EmirPosJurisdictionMissing.run(&[r], &ctx());
        assert!(out.is_empty());
    }

    #[test]
    fn does_not_fire_when_notional_is_none_or_zero() {
        let recs = vec![
            EmirPositionSetRecord {
                position_set_kind: Some("PosSet".into()),
                notional: None,
                underlying_id: None,
                ..Default::default()
            },
            EmirPositionSetRecord {
                position_set_kind: Some("PosSet".into()),
                notional: Some(Decimal::ZERO),
                underlying_id: None,
                ..Default::default()
            },
        ];
        let out = EmirPosJurisdictionMissing.run(&recs, &ctx());
        assert!(out.is_empty());
    }

    #[test]
    fn does_not_fire_on_collateral_kinds() {
        let r = EmirPositionSetRecord {
            position_set_kind: Some("CollPosSet".into()),
            notional: Some(Decimal::from(1000)),
            underlying_id: None,
            ..Default::default()
        };
        let out = EmirPosJurisdictionMissing.run(&[r], &ctx());
        assert!(out.is_empty());
    }
}
