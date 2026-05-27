//! EMIR Financial Instrument Reporting Status Advice
//! (`auth.031`) granular checks — v0.20 A5.
//!
//! Per-record envelope sanity check on
//! [`crate::EmirStatusAdviceRecord`]. The auth.031 message
//! carries no derivatives payload (it is a TR -> firm ack), so
//! the only check that fires here verifies the envelope
//! identity: an ack is only meaningful if it carries the
//! submission identifier being acked, a status, AND a
//! timestamp.
//!
//! See `docs/auth-messages/emir-auth031.md` for the rationale
//! behind shipping a single envelope check rather than a richer
//! suite.

use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, EmirStatusAdviceRecord, Severity};

/// An EMIR auth.031 ack envelope check.
pub trait EmirStatusAdviceCheck: Send + Sync {
    /// Stable identifier, e.g. `EMIR.ACK.ENVELOPE_WELLFORMED`.
    fn id(&self) -> &'static str;
    /// The DQ dimension this check belongs to.
    fn dimension(&self) -> DqDimension;
    /// Default severity for issues raised by this check.
    fn severity(&self) -> Severity;
    /// Execute the check over the ack envelope records slice.
    fn run(&self, records: &[EmirStatusAdviceRecord], ctx: &CheckContext) -> Vec<DqIssue>;
}

mod envelope_wellformed;

pub use envelope_wellformed::EmirStatusAdviceEnvelopeWellformed;
