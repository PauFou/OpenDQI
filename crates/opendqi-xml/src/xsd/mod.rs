//! XSD validation abstractions.
//!
//! [`XsdValidator`] is implemented by [`ExternalXmllintValidator`]
//! (shells out to `xmllint --noout --schema`) and by [`NoopValidator`]
//! (always returns no violations — handy in tests and when no schema
//! has been requested).
//!
//! The trait splits two outcome kinds:
//!
//! - `Ok(Vec<XsdViolation>)`: the document was checked. An empty vec
//!   means it conforms; entries describe per-line schema violations.
//! - `Err(XsdToolError)`: the tool or schema itself failed (xmllint
//!   missing, schema file unparseable, IO error). The caller should
//!   surface this as a single warning and continue with the next file.

mod external_xmllint;
mod noop;

use std::path::Path;

pub use external_xmllint::ExternalXmllintValidator;
pub use noop::NoopValidator;

/// One schema validity violation, normally one per error line emitted
/// by the underlying validator.
#[derive(Debug, Clone)]
pub struct XsdViolation {
    /// 1-based line in the source XML, when available.
    pub line: Option<u32>,
    /// 1-based column, when available.
    pub column: Option<u32>,
    /// Human-readable violation message, normally taken verbatim from
    /// the underlying tool.
    pub message: String,
}

/// Operational failure of the XSD validation tool itself. This is not
/// a property of the document under test.
#[derive(Debug, Clone, thiserror::Error)]
#[error("XSD tool error: {message}")]
pub struct XsdToolError {
    /// Description of the underlying problem.
    pub message: String,
}

impl XsdToolError {
    /// Build a tool error from any string-like message.
    pub fn new<S: Into<String>>(message: S) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// Validate an XML file against a schema.
pub trait XsdValidator: Send + Sync {
    /// Validate `xml_path`. See module docs for the meaning of the
    /// return value.
    fn validate(&self, xml_path: &Path) -> Result<Vec<XsdViolation>, XsdToolError>;
}
