//! EMIR Trade State Report (TSR) checks. State-oriented: each check
//! takes the TR's snapshot of outstanding trades and produces
//! issues about freshness, completeness, lifecycle anomalies, and
//! duplicates.

mod active_past_maturity;
mod duplicate_active_uti;
mod missing_valuation;
mod outstanding_summary;
mod placeholder_maturity;
mod stale_valuation;
mod valuation_after_termination;

pub use active_past_maturity::EmirActivePastMaturity;
pub use duplicate_active_uti::EmirDuplicateActiveUti;
pub use missing_valuation::EmirMissingValuationOnTsr;
pub use outstanding_summary::EmirOutstandingSummary;
pub use placeholder_maturity::EmirPlaceholderMaturity;
pub use stale_valuation::EmirStaleValuationOnTsr;
pub use valuation_after_termination::EmirTsrValuationAfterTermination;

/// Returns true when the record's `status` field (case-insensitive)
/// looks "active" — i.e. the trade should still be live. Empty /
/// missing status defaults to active (a TSR with no explicit status
/// is implicitly an outstanding-trade report).
pub(crate) fn is_outstanding(r: &crate::model::TrStateRecord) -> bool {
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
