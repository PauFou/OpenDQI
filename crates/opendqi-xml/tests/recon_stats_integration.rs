//! End-to-end test for the EMIR `auth.091` (Reconciliation Statistics)
//! pipeline against the **schema-shaped** fixture (real ESMA
//! `auth.091.001.02` element paths; pairing/recon rates **derived** by
//! accumulating cohort `TtlNbOfTxs` by `Pairg`/`Rcncltn`). See
//! `docs/auth-messages/emir-auth091.md`.
//!
//! `EMIR.RST.OUTSTANDING_UNPAIRED_HIGH` is intentionally NOT asserted:
//! real auth.091 has no outstanding-paired/unpaired field, so that
//! check is unreachable from this message (documented limitation).

use std::collections::BTreeSet;
use std::path::PathBuf;

use opendqi_core::dq::{default_recon_stats_checks, run_all_recon_stats, CheckContext};
use opendqi_xml::read_emir_recon_stats_xml;

fn example(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join(rel)
}

#[test]
fn derives_rates_and_fires_low_rate_checks() {
    let outcome =
        read_emir_recon_stats_xml(&example("examples/emir/recon_stats/auth091-sample.xml"))
            .expect("parse");
    assert!(
        outcome.issues.is_empty(),
        "expected no format issues, got {:?}",
        outcome.issues
    );
    // One record per counterparty LEI, sorted: A, B, C.
    assert_eq!(outcome.records.len(), 3);
    let a = &outcome.records[0];
    assert_eq!(a.counterparty_lei.as_deref(), Some("LEIA0000000000000001"));
    assert_eq!(a.pairing_rate.unwrap().to_string(), "0.70");
    assert_eq!(a.recon_rate.unwrap().to_string(), "0.50");
    assert_eq!(
        a.reporting_date.map(|d| d.to_string()).as_deref(),
        Some("2026-05-13")
    );
    assert!(a.outstanding_unpaired.is_none(), "no source in auth.091");
    // LEI-C came only via a NoRptgRqrmnt cohort → no derived rates.
    let c = &outcome.records[2];
    assert_eq!(c.counterparty_lei.as_deref(), Some("LEIC0000000000000003"));
    assert!(c.pairing_rate.is_none() && c.recon_rate.is_none());

    let ctx = CheckContext::now_with_defaults();
    let issues = run_all_recon_stats(&default_recon_stats_checks(), &outcome.records, &[], &ctx);
    let ids: BTreeSet<&str> = issues.iter().map(|i| i.check_id.as_str()).collect();
    assert!(ids.contains("EMIR.RST.PAIRING_RATE_LOW"), "got {ids:?}");
    assert!(ids.contains("EMIR.RST.RECON_RATE_LOW"), "got {ids:?}");
    // Unreachable from real auth.091 (no outstanding-* field); and the
    // trend check needs a prior batch.
    assert!(!ids.contains("EMIR.RST.OUTSTANDING_UNPAIRED_HIGH"));
    assert!(!ids.contains("EMIR.RST.PAIRING_RATE_TREND_DOWN"));
}

#[test]
fn trend_check_fires_against_prior_batch() {
    let cur = read_emir_recon_stats_xml(&example("examples/emir/recon_stats/auth091-sample.xml"))
        .expect("parse cur");
    let prior = read_emir_recon_stats_xml(&example("examples/emir/recon_stats/auth091-prior.xml"))
        .expect("parse prior");
    let ctx = CheckContext::now_with_defaults();
    let issues = run_all_recon_stats(
        &default_recon_stats_checks(),
        &cur.records,
        &prior.records,
        &ctx,
    );
    let trend: Vec<_> = issues
        .iter()
        .filter(|i| i.check_id == "EMIR.RST.PAIRING_RATE_TREND_DOWN")
        .collect();
    assert_eq!(
        trend.len(),
        1,
        "LEI-A pairing 0.95 (prior) → 0.70 (current) is a 25pp drop"
    );
}

#[test]
fn no_activity_report_yields_zero_records_and_info_note() {
    let outcome =
        read_emir_recon_stats_xml(&example("examples/emir/recon_stats/auth091-no-records.xml"))
            .expect("parse");
    assert!(outcome.records.is_empty());
    assert_eq!(outcome.issues.len(), 1);
    assert_eq!(outcome.issues[0].check_id, "EMIR.FMT.RST_NO_RECORDS");
}
