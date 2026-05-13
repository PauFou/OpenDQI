//! OpenDQI reporting outputs: JSON, CSV, HTML.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod csv_out;
pub mod html_out;
pub mod json_out;

pub use csv_out::write_issues_csv;
pub use html_out::write_report_html;
pub use json_out::write_summary_json;
