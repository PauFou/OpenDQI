//! EMIR Derivatives Trade Report Query (`auth.029`) granular
//! checks — v0.20 A5.
//!
//! Per-record envelope sanity check on
//! [`crate::EmirQueryRecord`]. The auth.029 message carries no
//! derivatives payload (it is a firm-side query the firm sends
//! *to* the TR), so the only check that fires here verifies the
//! envelope identity: a query is only meaningful if it carries
//! at least a query id AND the LEI of the requesting firm.
//!
//! See `docs/auth-messages/emir-auth029.md` for the rationale
//! behind shipping a single envelope check rather than a richer
//! suite.

use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, EmirQueryRecord, Severity};

/// An EMIR auth.029 query envelope check.
pub trait EmirQueryCheck: Send + Sync {
    /// Stable identifier, e.g. `EMIR.QRY.ENVELOPE_WELLFORMED`.
    fn id(&self) -> &'static str;
    /// The DQ dimension this check belongs to.
    fn dimension(&self) -> DqDimension;
    /// Default severity for issues raised by this check.
    fn severity(&self) -> Severity;
    /// Execute the check over the query envelope records slice.
    fn run(&self, records: &[EmirQueryRecord], ctx: &CheckContext) -> Vec<DqIssue>;
}

mod envelope_wellformed;

pub use envelope_wellformed::EmirQueryEnvelopeWellformed;
