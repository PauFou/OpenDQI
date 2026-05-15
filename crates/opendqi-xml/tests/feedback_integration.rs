//! End-to-end test for the EMIR `auth.092` (rejection statistics)
//! pipeline against the schema-shaped fixture (real ESMA
//! `auth.092.001.04` element paths, synthetic values): parse the XML,
//! run the feedback check pack, assert `EMIR.FBK.TR_REJECTED_UTI`
//! fires. Mirrors `tr_state_integration.rs`. See
//! `docs/auth-messages/emir-auth092.md`.
//!
//! `EMIR.FBK.TR_MISSING_DESPITE_SUBMISSION`,
//! `EMIR.FBK.TR_MISSING_BUT_NOT_SENT` and
//! `EMIR.FBK.TR_INACCURATE_REPORTED` are intentionally NOT asserted:
//! real auth.092 has no Missing/Inaccurate branch, so those checks are
//! unreachable from this message (documented limitation).

use std::collections::BTreeSet;
use std::path::PathBuf;

use opendqi_core::dq::{default_feedback_checks, run_all_feedback, CheckContext};
use opendqi_core::FeedbackType;
use opendqi_xml::read_emir_feedback_xml;

fn example(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join(rel)
}

#[test]
fn schema_shaped_fixture_fires_tr_rejected_uti() {
    let outcome = read_emir_feedback_xml(&example("examples/emir/feedback/auth092-sample.xml"))
        .expect("parse");
    assert!(
        outcome.issues.is_empty(),
        "expected no format issues, got {:?}",
        outcome.issues
    );
    // 2 rejected (RJCT, INCF); the ACPT row is not a feedback record.
    assert_eq!(outcome.records.len(), 2, "ACPT skipped");
    let r0 = &outcome.records[0];
    assert_eq!(r0.uti.as_deref(), Some("OPENDQI-FBK-RJCT-0001"));
    assert_eq!(r0.feedback_type, FeedbackType::Rejected);
    // Faithful: the full DtldVldtnRule list; reason_code = the first.
    assert_eq!(r0.validation_rule_codes, vec!["VR-0001", "VR-0042"]);
    assert_eq!(r0.reason_code.as_deref(), Some("VR-0001"));
    assert_eq!(
        r0.reason_description.as_deref(),
        Some("Notional amount missing")
    );
    assert!(r0.feedback_timestamp.is_some());
    // The single-rule INCF record keeps a one-element list.
    assert_eq!(outcome.records[1].validation_rule_codes, vec!["VR-0100"]);

    let ctx = CheckContext::now_with_defaults();
    let issues = run_all_feedback(&default_feedback_checks(), &outcome.records, &[], &ctx);
    let ids: BTreeSet<&str> = issues.iter().map(|i| i.check_id.as_str()).collect();
    assert!(
        ids.contains("EMIR.FBK.TR_REJECTED_UTI"),
        "expected EMIR.FBK.TR_REJECTED_UTI to fire; got {ids:?}"
    );
}

#[test]
fn no_activity_report_yields_zero_records_and_info_note() {
    let outcome = read_emir_feedback_xml(&example("examples/emir/feedback/auth092-no-records.xml"))
        .expect("parse");
    assert!(outcome.records.is_empty());
    assert_eq!(outcome.issues.len(), 1);
    assert_eq!(outcome.issues[0].check_id, "EMIR.FMT.FBK_NO_RECORDS");
}
