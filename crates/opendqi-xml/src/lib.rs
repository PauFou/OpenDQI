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
pub mod emir_mar;
pub mod emir_msr;
pub mod emir_position_set;
pub mod emir_query;
pub mod emir_recon_stats;
pub mod emir_warnings;
pub mod feedback;
pub mod reconciliation;
pub mod sftr;
pub mod sftr_margin_activity;
pub mod sftr_margin_state;
pub mod sftr_missing_collateral;
pub mod sftr_reuse_activity;
pub mod sftr_reuse_state;
pub mod sftr_tr_state;
pub mod sftr_tr_status_advice;
pub mod tr_state;
pub mod wellformed;
pub mod xsd;

pub use emir::{read_emir_xml, XmlReadOutcome};
pub use emir_mar::{read_emir_mar_xml, MarXmlReadOutcome};
pub use emir_msr::{read_emir_msr_xml, MsrXmlReadOutcome};
pub use emir_position_set::{read_emir_position_set_xml, EmirPositionSetXmlReadOutcome};
pub use emir_query::{read_emir_query_xml, EmirQueryXmlReadOutcome};
pub use emir_recon_stats::{read_emir_recon_stats_xml, ReconStatsXmlReadOutcome};
pub use emir_warnings::{read_emir_warnings_xml, WarningsXmlReadOutcome};
pub use feedback::{read_emir_feedback_xml, FeedbackXmlReadOutcome};
pub use reconciliation::{read_sftr_reconciliation_xml, ReconciliationXmlReadOutcome};
pub use sftr::{read_sftr_xml, SftrXmlReadOutcome};
pub use sftr_margin_activity::{read_sftr_margin_activity_xml, SftrMarginActivityXmlReadOutcome};
pub use sftr_margin_state::{read_sftr_margin_state_xml, SftrMarginStateXmlReadOutcome};
pub use sftr_missing_collateral::{
    read_sftr_missing_collateral_xml, MissingCollateralXmlReadOutcome,
};
pub use sftr_reuse_activity::{read_sftr_reuse_activity_xml, SftrReuseActivityXmlReadOutcome};
pub use sftr_reuse_state::{read_sftr_reuse_state_xml, SftrReuseStateXmlReadOutcome};
pub use sftr_tr_state::{read_sftr_tr_state_xml, SftrTrStateXmlReadOutcome};
pub use sftr_tr_status_advice::{read_sftr_tr_status_advice_xml, SftrTrStatusAdviceXmlReadOutcome};
pub use tr_state::{read_emir_tr_state_xml, TrStateXmlReadOutcome};
pub use wellformed::{check_wellformedness, WellformednessError};
pub use xsd::{ExternalXmllintValidator, NoopValidator, XsdToolError, XsdValidator, XsdViolation};
