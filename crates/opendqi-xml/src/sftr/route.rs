//! Namespace-aware dispatcher for SFTR XML inputs.
//!
//! Currently the only supported format is ISO 20022 `auth.052.001.02`.
//! Any other root namespace produces a critical format issue.

use std::path::Path;

use opendqi_core::{DqDimension, DqIssue, Regime, Severity};
use quick_xml::events::Event;
use quick_xml::name::ResolveResult;
use quick_xml::NsReader;

use super::SftrXmlReadOutcome;
use crate::wellformed::check_wellformedness;

const ISO20022_AUTH_052_NS: &[u8] = b"urn:iso:std:iso:20022:tech:xsd:auth.052.001.02";

const CHECK_NOT_WELLFORMED: &str = "SFTR.FMT.XML_NOT_WELLFORMED";
const CHECK_UNSUPPORTED_NS: &str = "SFTR.FMT.XML_UNSUPPORTED_NAMESPACE";

/// Read an SFTR XML file. The format is auto-detected from the root
/// element's namespace.
pub fn read_sftr_xml(path: &Path) -> anyhow::Result<SftrXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();

    if let Err(err) = check_wellformedness(path) {
        return Ok(SftrXmlReadOutcome {
            records: vec![],
            issues: vec![DqIssue {
                check_id: CHECK_NOT_WELLFORMED.into(),
                regime: Regime::Sftr,
                severity: Severity::Critical,
                dimension: DqDimension::Validity,
                record_id: None,
                uti: None,
                field: None,
                value: None,
                message: format!("XML is not well-formed: {}", err.message),
                source_file: Some(source_label),
                evidence: Vec::new(),
            }],
        });
    }

    match peek_root_namespace(path)? {
        Some(ns) if ns == ISO20022_AUTH_052_NS => super::iso20022::read(path),
        other => {
            let actual = other
                .as_deref()
                .map(|n| String::from_utf8_lossy(n).into_owned())
                .unwrap_or_else(|| "(none)".into());
            Ok(SftrXmlReadOutcome {
                records: vec![],
                issues: vec![DqIssue {
                    check_id: CHECK_UNSUPPORTED_NS.into(),
                    regime: Regime::Sftr,
                    severity: Severity::Warning,
                    dimension: DqDimension::Validity,
                    record_id: None,
                    uti: None,
                    field: None,
                    value: None,
                    message: format!(
                        "Root namespace is '{actual}', expected 'urn:iso:std:iso:20022:tech:xsd:auth.052.001.02'."
                    ),
                    source_file: Some(source_label),
                    evidence: Vec::new(),
                }],
            })
        }
    }
}

fn peek_root_namespace(path: &Path) -> anyhow::Result<Option<Vec<u8>>> {
    let mut reader = NsReader::from_file(path)?;
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    loop {
        match reader.read_resolved_event_into(&mut buf)? {
            (ns, Event::Start(_)) | (ns, Event::Empty(_)) => {
                return Ok(match ns {
                    ResolveResult::Bound(n) => Some(n.as_ref().to_owned()),
                    _ => None,
                });
            }
            (_, Event::Eof) => return Ok(None),
            _ => {}
        }
        buf.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn write_tmp(name: &str, content: &[u8]) -> PathBuf {
        let p =
            std::env::temp_dir().join(format!("opendqi-sftr-route-{}-{name}", std::process::id()));
        std::fs::write(&p, content).unwrap();
        p
    }

    #[test]
    fn malformed_xml_yields_critical_issue() {
        let p = write_tmp(
            "broken.xml",
            br#"<?xml version="1.0"?><Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.052.001.02"><Rpt><UnqTxIdr>X</UnqTxIdr></Rpt"#,
        );
        let out = read_sftr_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert!(out.records.is_empty());
        assert_eq!(out.issues.len(), 1);
        assert_eq!(out.issues[0].check_id, CHECK_NOT_WELLFORMED);
    }

    #[test]
    fn unsupported_namespace_yields_warning() {
        let p = write_tmp(
            "ns.xml",
            br#"<?xml version="1.0"?><Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.030.001.03"/>"#,
        );
        let out = read_sftr_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert!(out
            .issues
            .iter()
            .any(|i| i.check_id == CHECK_UNSUPPORTED_NS));
    }
}
