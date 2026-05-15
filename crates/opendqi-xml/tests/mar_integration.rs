//! End-to-end test for the EMIR `auth.108` (Margin Activity Report)
//! pipeline against the schema-shaped fixture (real ESMA EMIR REFIT
//! `auth.108.001.01` element paths, synthetic values): parse the XML,
//! run the eight `EMIR.MAR.*` checks, assert each fires. Mirrors
//! `tr_state_integration.rs`. See `docs/auth-messages/emir-auth108.md`.

use std::collections::BTreeSet;
use std::path::PathBuf;

use opendqi_core::dq::{default_margin_activity_checks, run_all_margin_activity, CheckContext};
use opendqi_xml::read_emir_mar_xml;

fn example(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join(rel)
}

#[test]
fn schema_shaped_fixture_fires_all_eight_mar_checks() {
    let outcome =
        read_emir_mar_xml(&example("examples/emir/mar/auth108-sample.xml")).expect("parse");
    assert!(
        outcome.issues.is_empty(),
        "expected no format issues, got {:?}",
        outcome.issues
    );
    assert_eq!(outcome.records.len(), 8, "8 <Rpt> records");

    let r0 = &outcome.records[0];
    assert_eq!(r0.uti.as_deref(), Some("OPENDQI-MAR-DELTA-0001"));
    assert_eq!(r0.counterparty_1.as_deref(), Some("RPTGCPARTY0000000001"));
    assert_eq!(r0.counterparty_2.as_deref(), Some("OTHRCPARTY0000000002"));
    assert_eq!(r0.action_type.as_deref(), Some("MRGN"));
    assert_eq!(r0.margin_currency.as_deref(), Some("EUR"));
    assert_eq!(r0.collateral_portfolio_code.as_deref(), Some("PF-DELTA"));
    assert!(r0.reporting_timestamp.is_some());
    assert!(r0.event_timestamp.is_some());
    assert!(r0.collateral_haircut.is_none(), "no haircut % in auth.108");

    let ctx = CheckContext::now_with_defaults();
    let issues = run_all_margin_activity(
        &default_margin_activity_checks(),
        &outcome.records,
        &[],
        &ctx,
    );
    let ids: BTreeSet<&str> = issues.iter().map(|i| i.check_id.as_str()).collect();

    for expected in [
        "EMIR.MAR.MARGIN_TYPE_ENUM",
        "EMIR.MAR.POSTED_NEGATIVE",
        "EMIR.MAR.COLLECTED_NEGATIVE",
        "EMIR.MAR.LARGE_MARGIN_DELTA",
        "EMIR.MAR.MARGIN_NEEDS_CURRENCY",
        "EMIR.MAR.PORTFOLIO_CODE_MISSING",
        "EMIR.MAR.TIMELINESS",
        "EMIR.MAR.DUPLICATE_MARGIN_CALL",
    ] {
        assert!(
            ids.contains(expected),
            "expected {expected} to fire on the schema-shaped fixture; got {ids:?}"
        );
    }
}

#[test]
fn no_activity_report_yields_zero_records_and_info_note() {
    let outcome =
        read_emir_mar_xml(&example("examples/emir/mar/auth108-no-records.xml")).expect("parse");
    assert!(outcome.records.is_empty());
    assert_eq!(outcome.issues.len(), 1);
    assert_eq!(outcome.issues[0].check_id, "EMIR.FMT.MAR_NO_RECORDS");
}
