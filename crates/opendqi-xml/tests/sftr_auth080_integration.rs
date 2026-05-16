//! End-to-end test for the real SFTR `auth.080` (Reconciliation
//! Status Advice) pipeline, re-homed into the reconciliation family
//! (Milestone 0.4 increment 2): parse the schema-shaped fixture and
//! run the `SFTR.REC.*` checks. See
//! `docs/auth-messages/sftr-auth080.md`.
//!
//! `SFTR.REC.UNPAIRED_TRADE` is intentionally NOT asserted: "unpaired"
//! is only a `PairgRcncltnSts` summary count in real auth.080, not a
//! per-`RcncltnRpt` state (documented limitation). The `reconciliations`
//! store persist path is unchanged from auth.083 and is covered by
//! `opendqi-store` round-trip tests + the CLI smoke.

use std::collections::BTreeSet;
use std::path::PathBuf;

use opendqi_core::dq::{
    default_sftr_reconciliation_checks, run_all_sftr_reconciliation, CheckContext,
};
use opendqi_xml::read_sftr_reconciliation_xml;

fn example(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join(rel)
}

#[test]
fn real_auth080_fires_reconciliation_checks() {
    let outcome =
        read_sftr_reconciliation_xml(&example("examples/sftr/reconciliation/auth080-sample.xml"))
            .expect("parse");
    assert!(
        outcome.issues.is_empty(),
        "expected no format issues, got {:?}",
        outcome.issues
    );
    assert_eq!(outcome.records.len(), 3);

    let r1 = &outcome.records[1];
    assert_eq!(r1.uti.as_deref(), Some("OPENDQI-SFTR-RECON-NOTMTCHD-2"));
    assert_eq!(r1.reconciliation_status.as_deref(), Some("UNRECONCILED"));
    assert_eq!(
        r1.mismatched_fields,
        vec!["TermntnDt", "MtrtyDt", "CollValDt"]
    );

    let ctx = CheckContext::now_with_defaults();
    let issues = run_all_sftr_reconciliation(
        &default_sftr_reconciliation_checks(),
        &outcome.records,
        &[],
        &ctx,
    );
    let ids: BTreeSet<&str> = issues.iter().map(|i| i.check_id.as_str()).collect();
    assert!(ids.contains("SFTR.REC.UNRECONCILED_TRADE"), "got {ids:?}");
    assert!(ids.contains("SFTR.REC.FIELD_MISMATCH"), "got {ids:?}");
    // Unreachable from real auth.080 (unpaired is summary-only).
    assert!(!ids.contains("SFTR.REC.UNPAIRED_TRADE"));
}

#[test]
fn no_activity_report_yields_zero_records_and_info_note() {
    let outcome = read_sftr_reconciliation_xml(&example(
        "examples/sftr/reconciliation/auth080-no-records.xml",
    ))
    .expect("parse");
    assert!(outcome.records.is_empty());
    assert_eq!(outcome.issues.len(), 1);
    assert_eq!(outcome.issues[0].check_id, "SFTR.FMT.RCNCLN_NO_RECORDS");
}
