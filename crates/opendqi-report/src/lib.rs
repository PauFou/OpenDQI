//! OpenDQI reporting outputs: JSON, CSV, HTML.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod csv_out;
pub mod email;
pub mod html_out;
pub mod json_out;

pub use csv_out::{write_issues_csv, write_issues_csv_from_iter};
pub use email::{send_report_email, send_smtp_test, SmtpConfig};
pub use html_out::{write_report_html, TopIssues};
pub use json_out::write_summary_json;
