//! SFTR Missing Collateral Request (auth.083) checks.
//!
//! Operational, not statistical: every record is a TR→firm request to
//! provide the collateral missing for one SFT. The checks surface each
//! request as an actionable issue and flag requests that cannot be
//! reconciled because the message omitted the UTI. Distinct from the
//! SFTR reconciliation-statistics family.

use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, MissingCollateralRecord, Regime, Severity};

mod missing_collateral_requested;
mod missing_uti_on_request;

pub use missing_collateral_requested::SftrMcrMissingCollateralRequested;
pub use missing_uti_on_request::SftrMcrMissingUtiOnRequest;

/// An SFTR auth.083 Missing Collateral Request check.
pub trait MissingCollateralCheck: Send + Sync {
    /// Stable identifier, e.g. `SFTR.MCR.MISSING_COLLATERAL_REQUESTED`.
    fn id(&self) -> &'static str;
    /// The DQ dimension this check belongs to.
    fn dimension(&self) -> DqDimension;
    /// Default severity for issues raised by this check.
    fn severity(&self) -> Severity;
    /// Execute the check against the current batch.
    fn run(&self, records: &[MissingCollateralRecord], ctx: &CheckContext) -> Vec<DqIssue>;
}

/// Default SFTR auth.083 check registry (2 checks).
pub fn default_missing_collateral_checks() -> Vec<Box<dyn MissingCollateralCheck>> {
    vec![
        Box::new(SftrMcrMissingCollateralRequested),
        Box::new(SftrMcrMissingUtiOnRequest),
    ]
}

/// Build a `DqIssue` shell pre-populated with the record's common
/// fields. Callers override `field`, `value`, `message`.
pub(crate) fn build_issue(
    check_id: &str,
    severity: Severity,
    dimension: DqDimension,
    rec: &MissingCollateralRecord,
) -> DqIssue {
    DqIssue {
        check_id: check_id.into(),
        regime: Regime::Sftr,
        severity,
        dimension,
        record_id: rec.record_id.clone(),
        uti: rec.uti.clone(),
        field: None,
        value: None,
        message: String::new(),
        source_file: rec.source_file.clone(),
        evidence: Vec::new(),
    }
}

/// Counterparty pair label for issue messages.
pub(crate) fn counterparty_label(rec: &MissingCollateralRecord) -> String {
    format!(
        "{} vs {}",
        rec.reporting_counterparty
            .as_deref()
            .unwrap_or("(no RptgCtrPty)"),
        rec.other_counterparty
            .as_deref()
            .unwrap_or("(no OthrCtrPty)"),
    )
}
