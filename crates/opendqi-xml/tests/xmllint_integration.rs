//! Integration test for `ExternalXmllintValidator`.
//!
//! This test actually spawns `xmllint`. It is skipped (with a printed
//! note) when the binary is not on `PATH`, so it does not break minimal
//! CI environments.

use std::path::PathBuf;
use std::process::Command;

use opendqi_xml::{ExternalXmllintValidator, XsdValidator};

fn xmllint_available() -> bool {
    Command::new("xmllint")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn workspace_path(rel: &str) -> PathBuf {
    // tests run with CWD = crate dir, so walk up to workspace root
    let crate_dir = env!("CARGO_MANIFEST_DIR");
    let workspace = std::path::Path::new(crate_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root");
    workspace.join(rel)
}

#[test]
fn sample_xml_validates_against_canonical_schema() {
    if !xmllint_available() {
        println!("xmllint not available; skipping");
        return;
    }
    let schema = workspace_path("examples/emir/schemas/opendqi-emir-v0.1.xsd");
    let xml = workspace_path("examples/emir/sample.xml");
    let validator = ExternalXmllintValidator::new(schema);
    let outcome = validator.validate(&xml).expect("validator should run");
    assert!(
        outcome.is_empty(),
        "expected no violations on sample.xml, got {outcome:?}"
    );
}

#[test]
fn violates_schema_fixture_reports_violations() {
    if !xmllint_available() {
        println!("xmllint not available; skipping");
        return;
    }
    let schema = workspace_path("examples/emir/schemas/opendqi-emir-v0.1.xsd");
    let xml = workspace_path("examples/emir/violates-schema.xml");
    let validator = ExternalXmllintValidator::new(schema);
    let outcome = validator.validate(&xml).expect("validator should run");
    assert!(
        outcome.len() >= 3,
        "expected at least three violations, got {} ({outcome:?})",
        outcome.len()
    );
    // At least one violation should mention xs:decimal (the bad Notional).
    assert!(
        outcome.iter().any(|v| v.message.contains("xs:decimal")),
        "expected a decimal violation in {outcome:?}"
    );
}

#[test]
fn missing_schema_returns_tool_error() {
    if !xmllint_available() {
        println!("xmllint not available; skipping");
        return;
    }
    let schema = PathBuf::from("/this/path/definitely/does/not/exist.xsd");
    let xml = workspace_path("examples/emir/sample.xml");
    let validator = ExternalXmllintValidator::new(schema);
    let err = validator
        .validate(&xml)
        .expect_err("missing schema should be a tool error");
    assert!(!err.message.is_empty());
}
