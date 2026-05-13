//! XML well-formedness check.
//!
//! Streams the file through `quick_xml::Reader` and reports the first
//! parse error with its byte position. Never loads the whole file.

use std::path::Path;

use quick_xml::events::Event;
use quick_xml::Reader;

/// Information about the first well-formedness error encountered.
#[derive(Debug, Clone)]
pub struct WellformednessError {
    /// Byte offset (1-based) where the error was detected.
    pub byte_position: u64,
    /// Human-readable error message from the parser.
    pub message: String,
}

impl std::fmt::Display for WellformednessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "XML parse error at byte {}: {}",
            self.byte_position, self.message
        )
    }
}

impl std::error::Error for WellformednessError {}

/// Stream-parse `path` and return `Ok(())` if the file is well-formed,
/// or `Err(WellformednessError)` describing the first parse error.
pub fn check_wellformedness(path: &Path) -> Result<(), WellformednessError> {
    let mut reader = Reader::from_file(path).map_err(|e| WellformednessError {
        byte_position: 0,
        message: format!("cannot open file: {e}"),
    })?;
    reader.config_mut().trim_text(false);

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => return Ok(()),
            Ok(_) => {}
            Err(e) => {
                return Err(WellformednessError {
                    byte_position: reader.buffer_position(),
                    message: e.to_string(),
                });
            }
        }
        buf.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_tmp(name: &str, content: &[u8]) -> std::path::PathBuf {
        let p =
            std::env::temp_dir().join(format!("opendqi-wellformed-{}-{name}", std::process::id()));
        std::fs::write(&p, content).unwrap();
        p
    }

    #[test]
    fn well_formed_xml_is_accepted() {
        let p = write_tmp("ok.xml", br#"<?xml version="1.0"?><root><a/></root>"#);
        check_wellformedness(&p).expect("should accept well-formed XML");
        std::fs::remove_file(&p).unwrap();
    }

    #[test]
    fn malformed_xml_is_rejected_with_position() {
        let p = write_tmp("broken.xml", br#"<?xml version="1.0"?><root><a></root>"#);
        let err = check_wellformedness(&p).expect_err("should reject malformed XML");
        assert!(err.byte_position > 0);
        assert!(!err.message.is_empty());
        std::fs::remove_file(&p).unwrap();
    }
}
