//! SFTR XML ingestion.

pub mod iso20022;
mod route;

use opendqi_core::{DqIssue, SftrRecord};

pub use route::read_sftr_xml;

/// Outcome of reading one SFTR XML file: records extracted plus any
/// file-level format issues (e.g. malformedness, unsupported
/// namespace).
#[derive(Debug, Default)]
pub struct SftrXmlReadOutcome {
    /// Records extracted from the file.
    pub records: Vec<SftrRecord>,
    /// File-level data-quality issues.
    pub issues: Vec<DqIssue>,
}
