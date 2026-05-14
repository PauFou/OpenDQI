//! End-to-end test for the auth.091 (Reconciliation Statistics)
//! pipeline: parse the bundled synthetic XML, run the four
//! `EMIR.RST.*` checks against the records, and assert the expected
//! mix of issues fires.

use std::path::PathBuf;

use opendqi_core::dq::{default_recon_stats_checks, run_all_recon_stats, CheckContext};
use opendqi_xml::read_emir_recon_stats_xml;

fn fixture_path() -> PathBuf {
    let crate_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(crate_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join("examples/emir/recon_stats/auth091-sample.xml")
}

#[test]
fn fixture_extracts_four_recon_stat_records() {
    let outcome = read_emir_recon_stats_xml(&fixture_path()).expect("parse");
    assert!(
        outcome.issues.is_empty(),
        "expected no format issues, got {:?}",
        outcome.issues
    );
    assert_eq!(outcome.records.len(), 4);
    let leis: Vec<&str> = outcome
        .records
        .iter()
        .filter_map(|r| r.counterparty_lei.as_deref())
        .collect();
    assert_eq!(
        leis,
        vec![
            "LEI-COUNTERPARTY-001",
            "LEI-COUNTERPARTY-002",
            "LEI-COUNTERPARTY-003",
            "LEI-COUNTERPARTY-004",
        ]
    );
}

#[test]
fn fixture_triggers_each_emir_rst_check_once() {
    let outcome = read_emir_recon_stats_xml(&fixture_path()).expect("parse");
    let ctx = CheckContext::now_with_defaults();
    let issues = run_all_recon_stats(&default_recon_stats_checks(), &outcome.records, &[], &ctx);

    let by_id: std::collections::HashMap<&str, usize> = {
        let mut m: std::collections::HashMap<&str, usize> = Default::default();
        for i in &issues {
            *m.entry(i.check_id.as_str()).or_insert(0) += 1;
        }
        m
    };

    assert_eq!(
        by_id.get("EMIR.RST.PAIRING_RATE_LOW"),
        Some(&1),
        "expected 1 PAIRING_RATE_LOW (LEI-002), got {by_id:?}"
    );
    assert_eq!(
        by_id.get("EMIR.RST.RECON_RATE_LOW"),
        Some(&1),
        "expected 1 RECON_RATE_LOW (LEI-003), got {by_id:?}"
    );
    assert_eq!(
        by_id.get("EMIR.RST.OUTSTANDING_UNPAIRED_HIGH"),
        Some(&1),
        "expected 1 OUTSTANDING_UNPAIRED_HIGH (LEI-004), got {by_id:?}"
    );
    // No prior batch passed → trend check should fire 0 times.
    assert!(
        !by_id.contains_key("EMIR.RST.PAIRING_RATE_TREND_DOWN"),
        "trend check should not fire without a prior batch"
    );
}

#[test]
fn trend_check_fires_when_pairing_rate_drops() {
    let outcome = read_emir_recon_stats_xml(&fixture_path()).expect("parse");
    // Inject a prior batch where the healthy counterparty's pairing
    // rate was higher; the current 0.95 vs a prior 0.99 is only 4pp
    // (below 5pp threshold) → no fire. We bump the prior to 0.99
    // and the *bad* counterparty (LEI-002) had a previous high rate
    // — let's verify the drop catches it.
    use opendqi_core::ReconStatsRecord;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    let prior = vec![ReconStatsRecord {
        counterparty_lei: Some("LEI-COUNTERPARTY-002".into()),
        pairing_rate: Some(Decimal::from_str("0.95").unwrap()),
        ..Default::default()
    }];
    let ctx = CheckContext::now_with_defaults();
    let issues = run_all_recon_stats(
        &default_recon_stats_checks(),
        &outcome.records,
        &prior,
        &ctx,
    );
    let trend_hits: Vec<_> = issues
        .iter()
        .filter(|i| i.check_id == "EMIR.RST.PAIRING_RATE_TREND_DOWN")
        .collect();
    assert_eq!(
        trend_hits.len(),
        1,
        "expected 1 trend hit for LEI-002 (0.95 → 0.70), got {}",
        trend_hits.len()
    );
}
