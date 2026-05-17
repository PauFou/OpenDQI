//! End-to-end test for the EMIR `auth.106` (Data-Quality Warnings)
//! pipeline against the **schema-shaped** fixture (real ESMA
//! `auth.106.001.01` element paths; missing/outdated/abnormal rates
//! **derived** from the report-level counts). See
//! `docs/auth-messages/emir-auth106.md`.
//!
//! The per-counterparty `Wrnngs` aggregate is now modelled separately
//! (`WarningsCounterpartyRecord`); its values must NOT leak into the
//! report-level aggregate. The deeper per-UTI `TxDtls` level stays a
//! documented deferred subset.

use std::collections::BTreeSet;
use std::path::PathBuf;

use opendqi_core::dq::{
    default_warnings_checks, default_warnings_counterparty_checks, run_all_warnings,
    run_all_warnings_counterparty, CheckContext,
};
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
    // Report-level checks never carry the CTRPTY_ prefix.
    assert!(!ids.iter().any(|id| id.starts_with("EMIR.WRN.CTRPTY_")));

    // The per-counterparty `Wrnngs` block IS modelled separately, with
    // the huge per-CP values (kept out of the aggregate above).
    assert_eq!(outcome.counterparty_records.len(), 1);
    let cp = &outcome.counterparty_records[0];
    assert_eq!(cp.counterparty_lei.as_deref(), Some("LEIA0000000000000001"));
    assert_eq!(
        cp.reporting_date.map(|d| d.to_string()).as_deref(),
        Some("2026-05-13")
    );
    assert_eq!(cp.outstanding_derivatives, Some(999999));
    assert_eq!(cp.missing_valuation, Some(999999));
    assert_eq!(cp.outstanding_derivatives_margin, Some(888888));
    assert_eq!(cp.missing_margin_info, Some(888888));
    assert_eq!(cp.derivatives_reported, Some(777777));
    assert_eq!(cp.abnormal_values, Some(777777));

    let cp_issues = run_all_warnings_counterparty(
        &default_warnings_counterparty_checks(),
        &outcome.counterparty_records,
        &ctx,
    );
    let cp_ids: BTreeSet<&str> = cp_issues.iter().map(|i| i.check_id.as_str()).collect();
    // rate 1.0 on the four categories with a per-CP numerator+denominator.
    assert!(cp_ids.contains("EMIR.WRN.CTRPTY_MISSING_VALUATION_HIGH"));
    assert!(cp_ids.contains("EMIR.WRN.CTRPTY_OUTDATED_VALUATION_HIGH"));
    assert!(cp_ids.contains("EMIR.WRN.CTRPTY_MISSING_MARGIN_INFO_HIGH"));
    assert!(cp_ids.contains("EMIR.WRN.CTRPTY_ABNORMAL_VALUES_HIGH"));
    // No per-CP outdated-margin numerator in the sample → does not fire.
    assert!(!cp_ids.contains("EMIR.WRN.CTRPTY_OUTDATED_MARGIN_INFO_HIGH"));
}

#[test]
fn no_activity_report_yields_zero_records_and_info_note() {
    let outcome = read_emir_warnings_xml(&example("examples/emir/warnings/auth106-no-records.xml"))
        .expect("parse");
    assert!(outcome.records.is_empty());
    assert_eq!(outcome.issues.len(), 1);
    assert_eq!(outcome.issues[0].check_id, "EMIR.FMT.WRN_NO_RECORDS");
}
