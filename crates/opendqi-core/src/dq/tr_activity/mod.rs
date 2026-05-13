//! EMIR Trade Activity Report (TAR) checks. Activity-oriented:
//! detects repeated corrections, action-type spikes, duplicate
//! NEWTs in a single batch, and TAR↔TSR coherence anomalies.

mod duplicate_newt_in_batch;
mod newt_not_in_tsr;
mod repeated_correction;
mod spike_modi;
mod spike_term;

pub use duplicate_newt_in_batch::EmirDuplicateNewtInBatch;
pub use newt_not_in_tsr::EmirNewtNotInTsr;
pub use repeated_correction::EmirRepeatedCorrection;
pub use spike_modi::EmirSpikeModi;
pub use spike_term::EmirSpikeTerm;
