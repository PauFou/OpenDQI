//! End-to-end test for the SFTR `auth.084` (Transaction Status
//! Advice) parser against the schema-shaped fixture (real ESMA
//! SFTR `auth.084.001.02` element paths, synthetic values).
//!
//! v0.18 D1.

use std::path::PathBuf;

use opendqi_xml::read_sftr_tr_status_advice_xml;

fn example(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join(rel)
}

const FIXTURE: &str = "examples/sftr/tr_status_advice/auth084-sample.xml";

#[test]
fn fixture_parses_one_record_with_no_format_issues() {
    let outcome = read_sftr_tr_status_advice_xml(&example(FIXTURE)).expect("parse");
    assert!(
        outcome.issues.is_empty(),
        "expected no format issues, got {:?}",
        outcome.issues
    );
    assert_eq!(outcome.records.len(), 1, "1 <Rpt> record");
}

#[test]
fn aggregate_totals_match_fixture() {
    let outcome = read_sftr_tr_status_advice_xml(&example(FIXTURE)).expect("parse");
    let r = &outcome.records[0];
    assert_eq!(r.total_reports, Some(1000));
    assert_eq!(r.total_reports_accepted, Some(955));
    assert_eq!(r.total_reports_rejected, Some(45));
    // Sum of per-error counts equals total_reports_rejected.
    let per_err_sum: u64 = r.rejected_reports_per_error.values().sum();
    assert_eq!(per_err_sum, 45);
}

#[test]
fn per_error_breakdown_extracted() {
    let outcome = read_sftr_tr_status_advice_xml(&example(FIXTURE)).expect("parse");
    let r = &outcome.records[0];
    assert_eq!(r.rejected_reports_per_error.len(), 3);
    assert_eq!(r.rejected_reports_per_error.get("VR-001"), Some(&20));
    assert_eq!(r.rejected_reports_per_error.get("VR-002"), Some(&15));
    assert_eq!(r.rejected_reports_per_error.get("VR-099"), Some(&10));
}
