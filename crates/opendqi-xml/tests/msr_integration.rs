//! End-to-end test for the EMIR `auth.109` (Margin State Report)
//! pipeline against the schema-shaped fixture (real ESMA EMIR REFIT
//! `auth.109.001.01` element paths, synthetic values): parse the XML,
//! run the `EMIR.MSR.*` checks, assert the seven reachable ones fire.
//!
//! `EMIR.MSR.HAIRCUT_OUT_OF_RANGE` is intentionally NOT asserted:
//! `auth.109` carries no haircut percentage, so that check is
//! unreachable with real data (documented limitation — see
//! `docs/auth-messages/emir-auth109.md`).

use std::collections::BTreeSet;
use std::path::PathBuf;

use opendqi_core::dq::{default_margin_state_checks, run_all_margin_state, CheckContext};
use opendqi_xml::read_emir_msr_xml;

fn example(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join(rel)
}

#[test]
fn schema_shaped_fixture_fires_the_seven_reachable_msr_checks() {
    let outcome =
        read_emir_msr_xml(&example("examples/emir/msr/auth109-sample.xml")).expect("parse");
    assert!(
        outcome.issues.is_empty(),
        "expected no format issues, got {:?}",
        outcome.issues
    );
    assert_eq!(outcome.records.len(), 6, "6 <Stat> records");

    let r0 = &outcome.records[0];
    assert_eq!(r0.uti.as_deref(), Some("OPENDQI-MSR-NEGIM-0001"));
    assert_eq!(r0.counterparty_1.as_deref(), Some("RPTGCPARTY0000000001"));
    assert_eq!(r0.collateralization_category.as_deref(), Some("FLCL"));
    assert!(r0.haircut_applied.is_none(), "no haircut % in auth.109");
    let stale = &outcome.records[3];
    assert!(stale.state_as_of.is_some(), "state_as_of from RptgTmStmp");

    let ctx = CheckContext::now_with_defaults();
    let issues = run_all_margin_state(&default_margin_state_checks(), &outcome.records, &[], &ctx);
    let ids: BTreeSet<&str> = issues.iter().map(|i| i.check_id.as_str()).collect();

    for expected in [
        "EMIR.MSR.INITIAL_MARGIN_NEGATIVE",
        "EMIR.MSR.VARIATION_MARGIN_NEGATIVE",
        "EMIR.MSR.COLLATERAL_MARKET_VALUE_NEGATIVE",
        "EMIR.MSR.MARGIN_STALE",
        "EMIR.MSR.MARGIN_MISSING_FOR_OUTSTANDING",
        "EMIR.MSR.COLLATERALIZATION_CATEGORY_ENUM",
        "EMIR.MSR.IM_POSTED_VS_COLLECTED_IMBALANCE",
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
        read_emir_msr_xml(&example("examples/emir/msr/auth109-no-records.xml")).expect("parse");
    assert!(outcome.records.is_empty());
    assert_eq!(outcome.issues.len(), 1);
    assert_eq!(outcome.issues[0].check_id, "EMIR.FMT.MSR_NO_RECORDS");
}
