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

use opendqi_core::dq::{
    default_recon_stats_checks, default_reconciliation_checks, run_all_recon_stats,
    run_all_reconciliation, CheckContext,
};
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
fn emits_per_transaction_reconciliation_records_and_fires_emir_rec_checks() {
    let outcome =
        read_emir_recon_stats_xml(&example("examples/emir/recon_stats/auth091-sample.xml"))
            .expect("parse");
    assert!(outcome.issues.is_empty());
    // Rate derivation is unchanged: still one ReconStatsRecord per LEI.
    assert_eq!(outcome.records.len(), 3);

    // One ReconciliationRecord per TxDtls/RcncltnRpt, in document order.
    let recs = &outcome.reconciliation_records;
    assert_eq!(recs.len(), 6, "got {recs:#?}");

    let by_uti = |u: &str| recs.iter().find(|r| r.uti.as_deref() == Some(u)).unwrap();

    let a4 = by_uti("U-A-4");
    assert_eq!(a4.pairing_status.as_deref(), Some("UNPAIRED"));
    assert_eq!(a4.reconciliation_status.as_deref(), Some("UNRECONCILED"));
    assert_eq!(
        a4.other_counterparty.as_deref(),
        Some("LEIZ0000000000000099")
    );
    assert_eq!(
        a4.mismatched_fields,
        vec!["CtrctTp".to_string(), "CtrctVal".to_string()],
        "scalar + nested Val1!=Val2, document order"
    );
    // PARD/RECO with all MtchgCrit equal ⇒ clean.
    let b1 = by_uti("U-B-1");
    assert_eq!(b1.pairing_status.as_deref(), Some("PAIRED"));
    assert_eq!(b1.reconciliation_status.as_deref(), Some("RECONCILED"));
    assert!(b1.mismatched_fields.is_empty());
    // NoRptgRqrmnt cohort ⇒ no inherited status.
    let c1 = by_uti("U-C-1");
    assert!(c1.pairing_status.is_none() && c1.reconciliation_status.is_none());

    let ctx = CheckContext::now_with_defaults();
    let issues = run_all_reconciliation(
        &default_reconciliation_checks(),
        &outcome.reconciliation_records,
        &[],
        &ctx,
    );
    let count = |id: &str| issues.iter().filter(|i| i.check_id == id).count();
    // UNPAIRED: U-A-3, U-A-4. UNRECONCILED: U-A-2, U-A-4.
    // FIELD_MISMATCH: U-A-4 ×2 (CtrctTp, CtrctVal).
    assert_eq!(count("EMIR.REC.UNPAIRED_TRADE"), 2, "got {issues:#?}");
    assert_eq!(count("EMIR.REC.UNRECONCILED_TRADE"), 2, "got {issues:#?}");
    assert_eq!(count("EMIR.REC.FIELD_MISMATCH"), 2, "got {issues:#?}");
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
