//! EMIR XML ingestion.
//!
//! [`read_emir_xml`] peeks the document's root namespace and routes to
//! the correct extractor:
//!
//! - `urn:iso:std:iso:20022:tech:xsd:auth.030.001.03` → [`iso20022`]
//!   (official EMIR Refit DerivativesTradeReport format);
//! - `https://opendqi.org/schemas/emir/v0.1` → [`simplified`]
//!   (OpenDQI's lightweight format documented in `docs/xml-format.md`);
//! - any other namespace → falls back to [`simplified`] best-effort
//!   with a warning issue (existing behavior preserved).

pub mod iso20022;
mod route;
pub mod simplified;

use opendqi_core::DqIssue;
use opendqi_core::EmirRecord;

pub use route::read_emir_xml;

/// Outcome of reading one XML file.
///
/// Format-level issues (well-formedness, unsupported namespace, unknown
/// elements) are collected here. They are not the result of running a
/// `Check` against records — they describe the file itself.
#[derive(Debug, Default)]
pub struct XmlReadOutcome {
    /// Records extracted from the file (possibly empty).
    pub records: Vec<EmirRecord>,
    /// File-level data-quality issues.
    pub issues: Vec<DqIssue>,
}
