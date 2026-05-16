//! End-to-end test for the EMIR `auth.106` (Data-Quality Warnings)
//! pipeline against the **schema-shaped** fixture (real ESMA
//! `auth.106.001.01` element paths; missing/outdated/abnormal rates
//! **derived** from the report-level counts). See
//! `docs/auth-messages/emir-auth106.md`.
//!
//! The per-counterparty `Wrnngs` breakdown is a documented deferred
//! subset and must NOT leak into the report-level aggregate.

use std::collections::BTreeSet;
use std::path::PathBuf;

use opendqi_core::dq::{default_warnings_checks, run_all_warnings, CheckContext};
use opendqi_xml::read_emir_warnings_xml;

fn example(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join(rel)
}

#[test]
fn derives_rates_and_fires_high_rate_checks() {
    let outcome = read_emir_warnings_xml(&example("examples/emir/warnings/auth106-sample.xml"))
        .expect("parse");
    assert!(
        outcome.issues.is_empty(),
        "expected no format issues, got {:?}",
        outcome.issues
    );
    assert_eq!(outcome.records.len(), 1);
    let r = &outcome.records[0];
    assert_eq!(
        r.reporting_date.map(|d| d.to_string()).as_deref(),
        Some("2026-05-13")
    );
    // Report-level aggregate — the 999999/888888/777777 per-CtrPty
    // Wrnngs values must NOT have leaked in.
    assert_eq!(r.outstanding_derivatives, Some(1000));
    assert_eq!(r.missing_valuation, Some(80));
    assert_eq!(r.outstanding_derivatives_margin, Some(500));
    assert_eq!(r.derivatives_reported, Some(2000));
    assert_eq!(r.counterparty_lei, None, "per-CtrPty Wrnngs is deferred");
    assert_eq!(r.missing_valuation_rate.unwrap().to_string(), "0.08");
    assert_eq!(r.missing_margin_rate.unwrap().to_string(), "0.08");
    assert_eq!(r.abnormal_values_rate.unwrap().to_string(), "0.03");

    let ctx = CheckContext::now_with_defaults();
    let issues = run_all_warnings(&default_warnings_checks(), &outcome.records, &ctx);
    let ids: BTreeSet<&str> = issues.iter().map(|i| i.check_id.as_str()).collect();
    assert!(
        ids.contains("EMIR.WRN.MISSING_VALUATION_HIGH"),
        "got {ids:?}"
    );
    assert!(
        ids.contains("EMIR.WRN.MISSING_MARGIN_INFO_HIGH"),
        "got {ids:?}"
    );
    assert!(ids.contains("EMIR.WRN.ABNORMAL_VALUES_HIGH"), "got {ids:?}");
    // Below-threshold rates do not fire.
    assert!(!ids.contains("EMIR.WRN.OUTDATED_VALUATION_HIGH"));
    assert!(!ids.contains("EMIR.WRN.OUTDATED_MARGIN_INFO_HIGH"));
}

#[test]
fn no_activity_report_yields_zero_records_and_info_note() {
    let outcome = read_emir_warnings_xml(&example("examples/emir/warnings/auth106-no-records.xml"))
        .expect("parse");
    assert!(outcome.records.is_empty());
    assert_eq!(outcome.issues.len(), 1);
    assert_eq!(outcome.issues[0].check_id, "EMIR.FMT.WRN_NO_RECORDS");
}
