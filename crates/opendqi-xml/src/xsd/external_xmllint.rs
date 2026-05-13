//! XSD validation by shelling out to `xmllint --noout --schema`.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::{XsdToolError, XsdValidator, XsdViolation};

/// XSD validator backed by an external `xmllint` binary.
///
/// `xmllint` must be on `PATH`. Schema-load failures surface as
/// [`XsdToolError`]; document-level violations are returned in the
/// `Ok` payload.
#[derive(Debug, Clone)]
pub struct ExternalXmllintValidator {
    schema_path: PathBuf,
}

impl ExternalXmllintValidator {
    /// Build a validator targeting the schema at `schema_path`.
    pub fn new(schema_path: PathBuf) -> Self {
        Self { schema_path }
    }
}

impl XsdValidator for ExternalXmllintValidator {
    fn validate(&self, xml_path: &Path) -> Result<Vec<XsdViolation>, XsdToolError> {
        let output = match Command::new("xmllint")
            .arg("--noout")
            .arg("--schema")
            .arg(&self.schema_path)
            .arg(xml_path)
            .output()
        {
            Ok(out) => out,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(XsdToolError::new("xmllint not found on PATH"));
            }
            Err(e) => {
                return Err(XsdToolError::new(format!("could not run xmllint: {e}")));
            }
        };

        let stderr = String::from_utf8_lossy(&output.stderr);

        // Tool/schema errors take precedence over exit codes.
        if is_tool_error(&stderr) {
            return Err(XsdToolError::new(format!(
                "xmllint could not load or compile the schema {}: {}",
                self.schema_path.display(),
                first_line(&stderr)
            )));
        }

        match output.status.code() {
            Some(0) => Ok(Vec::new()),
            Some(3) => Ok(parse_xmllint_stderr(&stderr)),
            Some(code) => Err(XsdToolError::new(format!(
                "xmllint exited with code {code}: {}",
                first_line(&stderr)
            ))),
            None => Err(XsdToolError::new("xmllint was terminated by a signal")),
        }
    }
}

/// Tool-error signatures emitted by xmllint when the schema or input
/// path is bad. These are never legitimate document violations.
fn is_tool_error(stderr: &str) -> bool {
    stderr.contains("failed to load external entity")
        || stderr.contains("Schemas parser error")
        || stderr.contains("failed to compile")
}

fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or("").trim()
}

/// Parse xmllint stderr into structured violations.
///
/// xmllint emits one validity error per line:
///
/// ```text
/// path/to/doc.xml:5: element Trade: Schemas validity error : Element 'X': ...
/// ```
///
/// The trailing summary `path/to/doc.xml fails to validate` is
/// ignored. Lines that do not look like validity errors are skipped.
fn parse_xmllint_stderr(text: &str) -> Vec<XsdViolation> {
    text.lines().filter_map(parse_error_line).collect()
}

fn parse_error_line(line: &str) -> Option<XsdViolation> {
    let marker_pos = line.find("Schemas validity ")?;
    let after_marker = &line[marker_pos + "Schemas validity ".len()..];
    // `after_marker` starts with e.g. "error : Element 'X' ..."
    let message = after_marker
        .split_once(" : ")
        .map(|(_, rest)| rest)
        .unwrap_or(after_marker)
        .trim()
        .to_owned();
    let line_no = extract_line_number(&line[..marker_pos]);
    Some(XsdViolation {
        line: line_no,
        column: None,
        message,
    })
}

/// Pull the first all-digit `:N:` segment from a prefix like
/// `path/to/doc.xml:5: element X:`.
fn extract_line_number(prefix: &str) -> Option<u32> {
    prefix
        .split(':')
        .skip(1)
        .find_map(|s| s.trim().parse::<u32>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_extracts_line_and_message() {
        let stderr = "examples/emir/violates-schema.xml:7: element Notional: Schemas validity error : Element 'Notional': 'not_a_number' is not a valid value of the atomic type 'xs:decimal'.
examples/emir/violates-schema.xml fails to validate
";
        let v = parse_xmllint_stderr(stderr);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].line, Some(7));
        assert!(v[0].message.contains("not a valid value"));
        assert!(v[0].message.contains("xs:decimal"));
    }

    #[test]
    fn parse_skips_summary_and_blank_lines() {
        let stderr = "
file.xml fails to validate
warning: something innocuous

";
        let v = parse_xmllint_stderr(stderr);
        assert!(v.is_empty(), "got {v:?}");
    }

    #[test]
    fn parse_keeps_multiple_violations() {
        let stderr = "a.xml:3: element X: Schemas validity error : Element 'X': bad.
a.xml:10: element Y: Schemas validity error : Element 'Y': also bad.
a.xml fails to validate
";
        let v = parse_xmllint_stderr(stderr);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].line, Some(3));
        assert_eq!(v[1].line, Some(10));
    }

    #[test]
    fn tool_error_detection() {
        assert!(is_tool_error(
            "warning: failed to load external entity \"/nope.xsd\""
        ));
        assert!(is_tool_error("Schemas parser error : ...\n"));
        assert!(is_tool_error("WXS schema /nope.xsd failed to compile\n"));
        assert!(!is_tool_error(
            "doc.xml:1: element X: Schemas validity error : ...\n"
        ));
    }
}
