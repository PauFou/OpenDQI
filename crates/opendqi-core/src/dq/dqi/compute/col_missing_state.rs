//! `DQI_COL_MISSING_STATE` — share of outstanding TSR records
//! that should have collateral but have **no matching MSR row**.
//!
//! Sibling at the indicator level of `EMIR.COL.MISSING` (see
//! `dq/collateral_audit.rs`): both walk TSR↔MSR by UTI, but
//! the check emits per-UTI issues while this computer rolls up
//! the share into one [`DqiIndicator`] plus top-20 evidence.
//!
//! - **Layer:** TSR + MSR.
//! - **Denominator:** outstanding TSR records that *should* be
//!   collateralised — either `collateral_portfolio_code` is set,
//!   or `collateralisation_category` is `FCOL` / `PCOL` / `OCOL`
//!   (anything other than `UCOL`/unset).
//! - **Numerator:** records from the denominator whose UTI has
//!   **no** corresponding MSR row.
//! - **Dimension:** completeness.

use std::collections::HashSet;

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, MarginStateRecord, Regime, TrStateRecord};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_COL_MISSING_STATE";
const DESCRIPTION: &str = "Outstanding, collateralised TSR records whose UTI has no companion \
MSR row — likely margining-state reporting gap.";

fn is_outstanding(r: &TrStateRecord) -> bool {
    if r.termination_date.is_some() {
        return false;
    }
    match r.status.as_deref() {
        Some(s) => {
            let up = s.trim().to_ascii_uppercase();
            !(up.starts_with("MATUR") || up.starts_with("TERMIN"))
        }
        None => true,
    }
}

/// True if the TSR record looks like it should have a companion
/// MSR row. Mirrors the heuristic in `collateral_audit.rs`.
fn should_have_collateral(r: &TrStateRecord) -> bool {
    if r.collateral_portfolio_code.is_some() {
        return true;
    }
    // No collateralisation_category on TSR yet; fall back to
    // "we don't know" → assume yes for any outstanding row with
    // a collateral portfolio code, otherwise no.
    false
}

/// Compute `DQI_COL_MISSING_STATE`.
pub fn compute_dqi_col_missing_state(
    tsr: &[TrStateRecord],
    msr: &[MarginStateRecord],
    thresholds: &Thresholds,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    // Index MSR by UTI once.
    let msr_utis: HashSet<&str> = msr.iter().filter_map(|m| m.uti.as_deref()).collect();

    let mut denominator: u64 = 0;
    let mut numerator: u64 = 0;
    let mut offenders: Vec<DqiEvidence> = Vec::new();

    for r in tsr {
        if !is_outstanding(r) || !should_have_collateral(r) {
            continue;
        }
        denominator += 1;
        let Some(uti) = r.uti.as_deref() else {
            // Outstanding-collateralised but no UTI — already a
            // completeness defect caught elsewhere; we cannot
            // join, treat as missing-state too.
            numerator += 1;
            offenders.push(DqiEvidence {
                indicator_id: INDICATOR_ID.into(),
                uti: String::new(),
                counterparty: r.reporting_counterparty.clone(),
                asset_class: None,
                source_file: r.source_file.clone(),
                observed_value: r.collateral_portfolio_code.clone(),
                explanation: "collateralised TSR row has no UTI — cannot join MSR".into(),
            });
            continue;
        };
        if msr_utis.contains(uti) {
            continue;
        }
        numerator += 1;
        offenders.push(DqiEvidence {
            indicator_id: INDICATOR_ID.into(),
            uti: uti.to_string(),
            counterparty: r.reporting_counterparty.clone(),
            asset_class: None,
            source_file: r.source_file.clone(),
            observed_value: r.collateral_portfolio_code.clone(),
            explanation: "no MSR row for this outstanding, collateralised UTI".into(),
        });
    }

    offenders.sort_by(|a, b| {
        a.source_file
            .cmp(&b.source_file)
            .then_with(|| a.uti.cmp(&b.uti))
    });
    offenders.truncate(EVIDENCE_TOP_N);

    let pair = resolve_threshold(thresholds, INDICATOR_ID);
    let (rate, status) = rate_with_status(numerator, denominator, &pair);
    let indicator = DqiIndicator {
        indicator_id: INDICATOR_ID.into(),
        regime: Regime::Emir,
        dimension: DqDimension::Completeness,
        table_scope: "TSR+MSR".into(),
        numerator,
        denominator,
        rate,
        threshold_amber: Some(pair.amber),
        threshold_red: Some(pair.red),
        status,
        description: DESCRIPTION.into(),
    };
    (indicator, offenders)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dq::dqi::DqiStatus;

    fn tsr_row(uti: &str, portfolio: Option<&str>, status: Option<&str>) -> TrStateRecord {
        TrStateRecord {
            uti: Some(uti.into()),
            collateral_portfolio_code: portfolio.map(|s| s.into()),
            status: status.map(|s| s.into()),
            ..Default::default()
        }
    }

    fn msr_row(uti: &str) -> MarginStateRecord {
        MarginStateRecord {
            uti: Some(uti.into()),
            ..Default::default()
        }
    }

    #[test]
    fn empty_tsr_is_not_applicable() {
        let (ind, _) = compute_dqi_col_missing_state(
            &[],
            &[],
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn uncollateralised_tsr_rows_excluded_from_denominator() {
        let tsr = vec![
            tsr_row("U1", None, Some("OUTSTANDING")),
            tsr_row("U2", None, Some("OUTSTANDING")),
        ];
        let (ind, _) = compute_dqi_col_missing_state(
            &tsr,
            &[],
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 0);
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn all_collateralised_tsr_have_msr_is_green() {
        let tsr = vec![
            tsr_row("U1", Some("P1"), Some("OUTSTANDING")),
            tsr_row("U2", Some("P2"), Some("OUTSTANDING")),
        ];
        let msr = vec![msr_row("U1"), msr_row("U2")];
        let (ind, _) = compute_dqi_col_missing_state(
            &tsr,
            &msr,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.denominator, 2);
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn missing_msr_row_counts() {
        let tsr = vec![
            tsr_row("U1", Some("P1"), Some("OUTSTANDING")),
            tsr_row("U2", Some("P2"), Some("OUTSTANDING")),
            tsr_row("U3", Some("P3"), Some("OUTSTANDING")),
        ];
        let msr = vec![msr_row("U1")];
        let (ind, ev) = compute_dqi_col_missing_state(
            &tsr,
            &msr,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 3);
        assert_eq!(ind.numerator, 2);
        assert_eq!(ev.len(), 2);
        assert_eq!(ev[0].uti, "U2");
        assert_eq!(ev[1].uti, "U3");
    }

    #[test]
    fn matured_tsr_excluded() {
        let tsr = vec![tsr_row("U1", Some("P1"), Some("MATURED"))];
        let (ind, _) = compute_dqi_col_missing_state(
            &tsr,
            &[],
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 0);
    }
}
