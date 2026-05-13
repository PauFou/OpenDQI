//! Namespace-aware dispatcher for EMIR XML inputs.
//!
//! [`read_emir_xml`] checks well-formedness, peeks the root element's
//! namespace, and routes to:
//!
//! - [`super::iso20022`] for `urn:iso:std:iso:20022:tech:xsd:auth.030.001.03`;
//! - [`super::simplified`] for `https://opendqi.org/schemas/emir/v0.1`
//!   or any other namespace (best-effort with a warning).

use std::path::Path;

use opendqi_core::{DqDimension, DqIssue, Regime, Severity};
use quick_xml::events::Event;
use quick_xml::name::ResolveResult;
use quick_xml::NsReader;

use super::XmlReadOutcome;
use crate::wellformed::check_wellformedness;

const ISO20022_AUTH_030_NS: &[u8] = b"urn:iso:std:iso:20022:tech:xsd:auth.030.001.03";

const CHECK_NOT_WELLFORMED: &str = "EMIR.FMT.XML_NOT_WELLFORMED";

/// Read an EMIR XML file. The format is auto-detected from the root
/// element's namespace.
pub fn read_emir_xml(path: &Path) -> anyhow::Result<XmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();

    if let Err(err) = check_wellformedness(path) {
        return Ok(XmlReadOutcome {
            records: vec![],
            issues: vec![DqIssue {
                check_id: CHECK_NOT_WELLFORMED.into(),
                regime: Regime::Emir,
                severity: Severity::Critical,
                dimension: DqDimension::Validity,
                record_id: None,
                uti: None,
                field: None,
                value: None,
                message: format!("XML is not well-formed: {}", err.message),
                source_file: Some(source_label),
            }],
        });
    }

    match peek_root_namespace(path)? {
        Some(ns) if ns == ISO20022_AUTH_030_NS => super::iso20022::read(path),
        // Any other namespace (including v0.1 and unknown) routes to the
        // simplified extractor. simplified emits an UNSUPPORTED_NAMESPACE
        // warning itself when the namespace is not v0.1.
        _ => super::simplified::read(path),
    }
}

/// Read the first `Start` event from `path` and return its namespace
/// bytes (if any). The file is assumed to be well-formed.
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
        let p = std::env::temp_dir().join(format!("opendqi-route-{}-{name}", std::process::id()));
        std::fs::write(&p, content).unwrap();
        p
    }

    #[test]
    fn malformed_xml_yields_critical_issue_and_no_panic() {
        let p = write_tmp(
            "broken.xml",
            br#"<?xml version="1.0"?><EmirReport xmlns="https://opendqi.org/schemas/emir/v0.1"><Trade><UTI>X</UTI></Trad></EmirReport>"#,
        );
        let out = read_emir_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();

        assert!(out.records.is_empty());
        assert_eq!(out.issues.len(), 1);
        assert_eq!(out.issues[0].check_id, CHECK_NOT_WELLFORMED);
        assert_eq!(out.issues[0].severity, Severity::Critical);
    }

    #[test]
    fn routes_to_simplified_when_namespace_matches_v01() {
        let p = write_tmp(
            "v01.xml",
            br#"<?xml version="1.0"?>
<EmirReport xmlns="https://opendqi.org/schemas/emir/v0.1">
  <Trade><UTI>OPENDQI-ROUTE-1</UTI></Trade>
</EmirReport>"#,
        );
        let out = read_emir_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();

        assert_eq!(out.records.len(), 1);
        // No UNSUPPORTED_NAMESPACE issue should fire for v0.1.
        assert!(out
            .issues
            .iter()
            .all(|i| i.check_id != "EMIR.FMT.XML_UNSUPPORTED_NAMESPACE"));
    }

    #[test]
    fn routes_to_iso20022_when_namespace_matches() {
        let p = write_tmp(
            "iso.xml",
            br#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.030.001.03">
  <DerivsTradRpt>
    <TradData>
      <Rpt>
        <New>
          <CmonTradData>
            <TxData>
              <TradId><UnqTxIdr>OPENDQI-ROUTE-ISO-1</UnqTxIdr></TradId>
            </TxData>
          </CmonTradData>
        </New>
      </Rpt>
    </TradData>
  </DerivsTradRpt>
</Document>"#,
        );
        let out = read_emir_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();

        // No UNSUPPORTED_NAMESPACE because we routed to the ISO 20022 extractor.
        assert!(out
            .issues
            .iter()
            .all(|i| i.check_id != "EMIR.FMT.XML_UNSUPPORTED_NAMESPACE"));
        assert_eq!(out.records.len(), 1);
        assert_eq!(out.records[0].uti.as_deref(), Some("OPENDQI-ROUTE-ISO-1"));
    }

    #[test]
    fn unknown_namespace_falls_back_to_simplified_with_warning() {
        let p = write_tmp(
            "other.xml",
            br#"<?xml version="1.0"?>
<EmirReport xmlns="https://example.com/other/v9">
  <Trade><UTI>X</UTI></Trade>
</EmirReport>"#,
        );
        let out = read_emir_xml(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert!(out
            .issues
            .iter()
            .any(|i| i.check_id == "EMIR.FMT.XML_UNSUPPORTED_NAMESPACE"));
    }
}
