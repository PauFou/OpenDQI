//! XSD-conformance reliability gate.
//!
//! For every schema-verified message, this proves a **fully
//! XSD-valid** conformance instance (a) validates against the **real
//! ESMA XSD** via `xmllint` and (b) is ingested by the OpenDQI parser
//! producing records with no format issues — i.e. the parser handles
//! a real-schema-valid instance, not just the lean test shapes.
//!
//! **Local-only, opt-in, CI-skipped.** The real ESMA XSDs are
//! SWIFT-licensed and never committed (`ESMA_docs/` is gitignored), so
//! this gate cannot run in public CI. It activates only when the
//! developer points `OPENDQI_XSD_DIR` at a directory of extracted real
//! ESMA `.xsd` files **and** `xmllint` is on `PATH`; otherwise every
//! case prints a skip notice and passes as a no-op (so plain
//! `cargo test` / CI is unaffected). See `docs/xsd-validation.md`.
//!
//! Enable locally:
//! ```text
//! OPENDQI_XSD_DIR=/path/to/extracted/esma/xsds \
//!   cargo test -p opendqi-xml --test xsd_conformance
//! ```

use std::path::PathBuf;
use std::process::Command;

use opendqi_xml::{
    read_emir_feedback_xml, read_emir_mar_xml, read_emir_msr_xml, read_emir_recon_stats_xml,
    read_emir_tr_state_xml, read_emir_warnings_xml, read_emir_xml, read_sftr_reconciliation_xml,
    read_sftr_tr_state_xml, read_sftr_xml, ExternalXmllintValidator, XsdValidator,
};

fn xmllint_available() -> bool {
    Command::new("xmllint")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn workspace_path(rel: &str) -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join(rel)
}

/// Resolve the real ESMA XSD for `auth_prefix` (e.g. `auth.106`) in
/// `OPENDQI_XSD_DIR` by filename prefix (tolerates ESMA version
/// suffixes like `auth.106.001.01_ESMAUG_DATWRN_1.1.0.xsd`).
fn resolve_xsd(dir: &str, auth_prefix: &str) -> PathBuf {
    let mut matches: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("OPENDQI_XSD_DIR='{dir}' not readable: {e}"))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with(auth_prefix) && n.ends_with(".xsd"))
                .unwrap_or(false)
        })
        .collect();
    matches.sort();
    match matches.len() {
        0 => panic!(
            "no `{auth_prefix}*.xsd` in OPENDQI_XSD_DIR='{dir}' — extract the real ESMA XSD there"
        ),
        _ => matches.swap_remove(0),
    }
}

/// Returns the conformance fixture path when the gate is active and
/// the fixture validates cleanly against the real XSD; `None` when the
/// gate is opted out (env unset / xmllint absent) → caller no-ops.
/// Panics when active and the fixture is NOT XSD-valid (the point).
fn xsd_gate(auth_prefix: &str, fixture_rel: &str) -> Option<PathBuf> {
    let dir = match std::env::var("OPENDQI_XSD_DIR") {
        Ok(d) if !d.is_empty() => d,
        _ => {
            eprintln!("OPENDQI_XSD_DIR not set; skipping XSD conformance for {auth_prefix}");
            return None;
        }
    };
    if !xmllint_available() {
        eprintln!("xmllint not available; skipping XSD conformance for {auth_prefix}");
        return None;
    }
    let xsd = resolve_xsd(&dir, auth_prefix);
    let fixture = workspace_path(fixture_rel);
    assert!(
        fixture.exists(),
        "{auth_prefix}: conformance fixture missing: {}",
        fixture.display()
    );
    match ExternalXmllintValidator::new(xsd.clone()).validate(&fixture) {
        Ok(v) if v.is_empty() => Some(fixture),
        Ok(v) => {
            let detail = v
                .iter()
                .map(|x| format!("  L{:?}: {}", x.line, x.message))
                .collect::<Vec<_>>()
                .join("\n");
            panic!(
                "{auth_prefix}: conformance fixture is NOT XSD-valid against {} \
                 ({} violation(s)):\n{detail}",
                xsd.display(),
                v.len()
            );
        }
        Err(e) => panic!(
            "{auth_prefix}: xmllint tool error validating against {}: {}",
            xsd.display(),
            e.message
        ),
    }
}

/// One `#[test]` per message: XSD-validate the fixture, then prove the
/// parser round-trips it (records produced, no `*.FMT.XML_*` issue).
macro_rules! conformance {
    ($name:ident, $auth:literal, $fixture:literal, $parser:path) => {
        #[test]
        fn $name() {
            let Some(fx) = xsd_gate($auth, $fixture) else {
                return;
            };
            let out = $parser(&fx).expect(concat!($auth, ": parser failed"));
            assert!(
                !out.records.is_empty(),
                concat!($auth, ": valid instance produced zero records")
            );
            let fmt: Vec<String> = out
                .issues
                .iter()
                .filter(|i| i.check_id.contains(".FMT.XML_"))
                .map(|i| i.check_id.clone())
                .collect();
            assert!(
                fmt.is_empty(),
                "{}: parser raised format issues on a valid instance: {:?}",
                $auth,
                fmt
            );
        }
    };
}

// ---- EMIR Refit Outgoing Messages group ----------------------------
conformance!(
    auth030_tar,
    "auth.030",
    "examples/emir/conformance/auth030-valid.xml",
    read_emir_xml
);
conformance!(
    auth091_recon_stats,
    "auth.091",
    "examples/emir/conformance/auth091-valid.xml",
    read_emir_recon_stats_xml
);
conformance!(
    auth092_feedback,
    "auth.092",
    "examples/emir/conformance/auth092-valid.xml",
    read_emir_feedback_xml
);
conformance!(
    auth106_warnings,
    "auth.106",
    "examples/emir/conformance/auth106-valid.xml",
    read_emir_warnings_xml
);
conformance!(
    auth107_tr_state,
    "auth.107",
    "examples/emir/conformance/auth107-valid.xml",
    read_emir_tr_state_xml
);
conformance!(
    auth108_mar,
    "auth.108",
    "examples/emir/conformance/auth108-valid.xml",
    read_emir_mar_xml
);
conformance!(
    auth109_msr,
    "auth.109",
    "examples/emir/conformance/auth109-valid.xml",
    read_emir_msr_xml
);

// ---- SFTR group ----------------------------------------------------
conformance!(
    auth052_tar,
    "auth.052",
    "examples/sftr/conformance/auth052-valid.xml",
    read_sftr_xml
);
conformance!(
    auth079_tr_state,
    "auth.079",
    "examples/sftr/conformance/auth079-valid.xml",
    read_sftr_tr_state_xml
);
conformance!(
    auth080_reconciliation,
    "auth.080",
    "examples/sftr/conformance/auth080-valid.xml",
    read_sftr_reconciliation_xml
);
