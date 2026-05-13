//! OpenDQI XML ingestion.
//!
//! - [`wellformed::check_wellformedness`] performs a streaming
//!   well-formedness check on an XML file.
//! - [`emir::read_emir_xml`] extracts EMIR records from the simplified
//!   OpenDQI XML format (see `docs/xml-format.md`).
//!
//! Full ISO 20022 (auth.030 / auth.108) parsing and XSD validation are
//! planned for later milestones.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod emir;
pub mod feedback;
pub mod reconciliation;
pub mod sftr;
pub mod wellformed;
pub mod xsd;

pub use emir::{read_emir_xml, XmlReadOutcome};
pub use feedback::{read_emir_feedback_xml, read_sftr_feedback_xml, FeedbackXmlReadOutcome};
pub use reconciliation::{
    read_emir_reconciliation_xml, read_sftr_reconciliation_xml, ReconciliationXmlReadOutcome,
};
pub use sftr::{read_sftr_xml, SftrXmlReadOutcome};
pub use wellformed::{check_wellformedness, WellformednessError};
pub use xsd::{ExternalXmllintValidator, NoopValidator, XsdToolError, XsdValidator, XsdViolation};
