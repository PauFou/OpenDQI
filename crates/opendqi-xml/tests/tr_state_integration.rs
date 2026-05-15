//! End-to-end test for the EMIR `auth.107` (Trade State Report)
//! pipeline against the **schema-shaped** bundled fixture (real ESMA
//! EMIR REFIT `auth.107.001.01` element paths, synthetic values):
//! parse the XML, run the seven `EMIR.TST.*` state-health checks over
//! the parsed records, and assert every check fires as designed.
//! Proves the hardened parser still feeds the canonical model
//! correctly. See `docs/auth-messages/emir-auth107.md`.

use std::collections::BTreeSet;
use std::path::PathBuf;

use opendqi_core::dq::{default_tr_state_checks, run_all_tr_state, CheckContext};
use opendqi_xml::read_emir_tr_state_xml;

fn example(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join(rel)
}

#[test]
fn schema_shaped_fixture_fires_all_seven_tst_checks() {
    let outcome = read_emir_tr_state_xml(&example("examples/emir/tr_state/auth107-sample.xml"))
        .expect("parse");
    assert!(
        outcome.issues.is_empty(),
        "expected no format issues, got {:?}",
        outcome.issues
    );
    assert_eq!(outcome.records.len(), 8, "8 <Stat> records");

    // The hardened parser must have populated the canonical model from
    // the real auth.107 paths.
    let clean = &outcome.records[0];
    assert_eq!(clean.uti.as_deref(), Some("OPENDQI-TSR-CLEAN-0001"));
    assert_eq!(
        clean.reporting_counterparty.as_deref(),
        Some("12345678901234567890")
    );
    assert_eq!(
        clean.other_counterparty.as_deref(),
        Some("09876543210987654321")
    );
    assert_eq!(clean.notional_currency.as_deref(), Some("EUR"));
    assert_eq!(clean.notional_amount.unwrap().to_string(), "1000000.00");
    assert_eq!(clean.valuation_amount.unwrap().to_string(), "150.50");
    assert_eq!(clean.collateral_portfolio_code.as_deref(), Some("PORT-001"));
    assert!(clean.state_as_of.is_some(), "state_as_of from RptgTmStmp");
    assert!(clean.status.is_none(), "auth.107 has no status element");

    let ctx = CheckContext::now_with_defaults();
    let issues = run_all_tr_state(&default_tr_state_checks(), &outcome.records, &[], &ctx);
    let ids: BTreeSet<&str> = issues.iter().map(|i| i.check_id.as_str()).collect();

    for expected in [
        "EMIR.TST.OUTSTANDING_SUMMARY",
        "EMIR.TST.STALE_VALUATION",
        "EMIR.TST.MISSING_VALUATION",
        "EMIR.TST.ACTIVE_PAST_MATURITY",
        "EMIR.TST.PLACEHOLDER_MATURITY",
        "EMIR.TST.DUPLICATE_ACTIVE_UTI",
        "EMIR.TST.VALUATION_AFTER_TERMINATION",
    ] {
        assert!(
            ids.contains(expected),
            "expected {expected} to fire on the schema-shaped fixture; got {ids:?}"
        );
    }
}

#[test]
fn no_activity_report_yields_zero_records_and_info_note() {
    let outcome = read_emir_tr_state_xml(&example("examples/emir/tr_state/auth107-no-records.xml"))
        .expect("parse");
    assert!(outcome.records.is_empty());
    assert_eq!(outcome.issues.len(), 1);
    assert_eq!(outcome.issues[0].check_id, "EMIR.FMT.TSR_NO_RECORDS");
}
