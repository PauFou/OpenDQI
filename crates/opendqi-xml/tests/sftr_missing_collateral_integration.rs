//! End-to-end test for the SFTR `auth.083` (Missing Collateral
//! Request) pipeline against the **schema-shaped** lean fixture (real
//! ESMA `auth.083.001.02` element paths). See
//! `docs/auth-messages/sftr-auth083.md`.
//!
//! auth.083 is a TR→firm operational request: one `TxId` per SFT for
//! which the TR is asking the firm to supply the missing collateral.
//! `TxId` is mandatory ≥1 — there is no no-activity branch.

use std::collections::BTreeSet;
use std::path::PathBuf;

use opendqi_core::dq::{
    default_missing_collateral_checks, run_all_missing_collateral, CheckContext,
};
use opendqi_xml::read_sftr_missing_collateral_xml;

fn example(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join(rel)
}

#[test]
fn parses_requests_and_fires_mcr_checks() {
    let outcome = read_sftr_missing_collateral_xml(&example(
        "examples/sftr/missing_collateral/auth083-sample.xml",
    ))
    .expect("parse");
    assert!(
        outcome.issues.is_empty(),
        "expected no format issues, got {:?}",
        outcome.issues
    );
    assert_eq!(outcome.records.len(), 2);

    // Record 1: legal-entity other CP, UTI + master agreement.
    let r0 = &outcome.records[0];
    assert_eq!(
        r0.reporting_counterparty.as_deref(),
        Some("RPTGLEI0000000000001")
    );
    assert_eq!(
        r0.other_counterparty.as_deref(),
        Some("OTHRLEI0000000000002")
    );
    assert_eq!(r0.uti.as_deref(), Some("OPENDQI-SFTR-MCR-0001"));
    assert_eq!(r0.master_agreement_type.as_deref(), Some("GMRA"));
    assert_eq!(r0.master_agreement_version.as_deref(), Some("2011"));

    // Record 2: natural-person other CP, no UTI.
    let r1 = &outcome.records[1];
    assert_eq!(
        r1.other_counterparty.as_deref(),
        Some("NATURAL-PERSON-0002")
    );
    assert!(r1.uti.is_none());

    let ctx = CheckContext::now_with_defaults();
    let issues =
        run_all_missing_collateral(&default_missing_collateral_checks(), &outcome.records, &ctx);
    let ids: BTreeSet<&str> = issues.iter().map(|i| i.check_id.as_str()).collect();
    assert!(
        ids.contains("SFTR.MCR.MISSING_COLLATERAL_REQUESTED"),
        "got {ids:?}"
    );
    assert!(
        ids.contains("SFTR.MCR.MISSING_UTI_ON_REQUEST"),
        "got {ids:?}"
    );
    // One request issue per record (2), one missing-UTI (record 2).
    assert_eq!(
        issues
            .iter()
            .filter(|i| i.check_id == "SFTR.MCR.MISSING_COLLATERAL_REQUESTED")
            .count(),
        2
    );
    assert_eq!(
        issues
            .iter()
            .filter(|i| i.check_id == "SFTR.MCR.MISSING_UTI_ON_REQUEST")
            .count(),
        1
    );
}

#[test]
fn wrong_namespace_yields_format_warning_and_no_records() {
    let outcome = read_sftr_missing_collateral_xml(&example(
        "examples/sftr/reconciliation/auth080-sample.xml",
    ))
    .expect("parse");
    assert!(outcome.records.is_empty());
    assert_eq!(outcome.issues.len(), 1);
    assert_eq!(
        outcome.issues[0].check_id,
        "SFTR.FMT.XML_UNSUPPORTED_NAMESPACE"
    );
}
