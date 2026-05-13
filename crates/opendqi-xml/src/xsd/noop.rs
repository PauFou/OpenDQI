//! A no-op XSD validator. Always says the document is fine.

use std::path::Path;

use super::{XsdToolError, XsdValidator, XsdViolation};

/// Validator that performs no work and reports no violations.
///
/// Useful for tests and for code paths where `--xsd` is not provided
/// but a `Box<dyn XsdValidator>` is still expected.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopValidator;

impl XsdValidator for NoopValidator {
    fn validate(&self, _xml_path: &Path) -> Result<Vec<XsdViolation>, XsdToolError> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noop_returns_no_violations() {
        let out = NoopValidator
            .validate(Path::new("/does/not/matter"))
            .unwrap();
        assert!(out.is_empty());
    }
}
