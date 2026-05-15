//! End-to-end test for the SFTR `auth.079` (SFT Trade State Report)
//! pipeline against the schema-shaped fixture (real ESMA SFTR
//! `auth.079.001.02` element paths, synthetic values): parse the XML,
//! run the SFTR TSR check pack, assert every reachable
//! `SFTR.TST.*` / `SFTR.MSR.MGLD_*` check fires. Mirrors
//! `tr_state_integration.rs`. See `docs/auth-messages/sftr-auth079.md`.

use std::collections::BTreeSet;
use std::path::PathBuf;

use opendqi_core::dq::{default_sftr_tr_state_checks, run_all_sftr_tr_state, CheckContext};
use opendqi_xml::read_sftr_tr_state_xml;

fn example(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join(rel)
}

#[test]
fn schema_shaped_fixture_fires_all_reachable_sftr_tsr_checks() {
    let outcome = read_sftr_tr_state_xml(&example("examples/sftr/tr_state/auth079-sample.xml"))
        .expect("parse");
    assert!(
        outcome.issues.is_empty(),
        "expected no format issues, got {:?}",
        outcome.issues
    );
    assert_eq!(outcome.records.len(), 13, "13 <Stat> records");

    let r0 = &outcome.records[0];
    assert_eq!(r0.uti.as_deref(), Some("OPENDQI-SFTR-TSR-CLEAN-0001"));
    assert_eq!(r0.sft_type.as_deref(), Some("REPO"));
    assert_eq!(
        r0.reporting_counterparty.as_deref(),
        Some("RPTGCPARTY0000000001")
    );
    assert_eq!(
        r0.other_counterparty.as_deref(),
        Some("OTHRCPARTY0000000002")
    );
    assert_eq!(r0.loan_currency.as_deref(), Some("EUR"));
    assert_eq!(r0.collateral_value.unwrap().to_string(), "1050000.00");
    assert_eq!(r0.collateral_isin.as_deref(), Some("DE0001135275"));
    assert!(r0.state_as_of.is_some(), "state_as_of from RptgDtTm");
    assert!(r0.status.is_none(), "auth.079 has no status element");
    assert!(
        r0.collateral_portfolio_code.is_none(),
        "unmappable in auth.079"
    );

    let ctx = CheckContext::now_with_defaults();
    let issues =
        run_all_sftr_tr_state(&default_sftr_tr_state_checks(), &outcome.records, &[], &ctx);
    let ids: BTreeSet<&str> = issues.iter().map(|i| i.check_id.as_str()).collect();

    for expected in [
        "SFTR.TST.OUTSTANDING_SUMMARY",
        "SFTR.TST.MISSING_COLLATERAL",
        "SFTR.TST.ACTIVE_PAST_MATURITY",
        "SFTR.TST.HAIRCUT_OUT_OF_RANGE_ON_OUTSTANDING",
        "SFTR.TST.DUPLICATE_ACTIVE_UTI",
        "SFTR.TST.STALE_VALUATION",
        "SFTR.MSR.MGLD_OUTSTANDING_NEEDS_LOAN",
        "SFTR.MSR.MGLD_HAIRCUT_OUT_OF_RANGE",
        "SFTR.MSR.MGLD_COLLATERAL_UNDER_LOAN",
        "SFTR.MSR.MGLD_REUSE_REQUIRES_PORTFOLIO",
        "SFTR.MSR.MGLD_LOAN_COLL_CURRENCY_MISMATCH",
        "SFTR.MSR.MGLD_MISSING_ISIN",
    ] {
        assert!(
            ids.contains(expected),
            "expected {expected} to fire on the schema-shaped fixture; got {ids:?}"
        );
    }
}

#[test]
fn no_activity_report_yields_zero_records_and_info_note() {
    let outcome = read_sftr_tr_state_xml(&example("examples/sftr/tr_state/auth079-no-records.xml"))
        .expect("parse");
    assert!(outcome.records.is_empty());
    assert_eq!(outcome.issues.len(), 1);
    assert_eq!(outcome.issues[0].check_id, "SFTR.FMT.SFTR_TSR_NO_RECORDS");
}
