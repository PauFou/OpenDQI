//! End-to-end test of the rejection-profile → pre-submission loop.
//! Loads the bundled synthetic `rejection_profile/sample.yml`, scans
//! a tiny slice of EMIR records, and asserts that both
//! `EMIR.PSC.REPEATED_REJECTION` and `EMIR.PSC.LIKELY_REJECTION_PATTERN`
//! fire on the expected records.

use std::path::PathBuf;

use opendqi_core::dq::{default_pre_submission_checks, run_all_pre_submission, CheckContext};
use opendqi_core::{EmirRecord, RejectionProfileFile};
use rust_decimal::Decimal;

fn fixture_path() -> PathBuf {
    let crate_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(crate_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join("examples/emir/rejection_profile/sample.yml")
}

#[test]
fn fixture_deserialises_into_rejection_profile() {
    let text = std::fs::read_to_string(fixture_path()).expect("read fixture");
    let file: RejectionProfileFile = serde_yaml::from_str(&text).expect("parse profile");
    let p = file.profile;
    assert_eq!(p.total_feedbacks, 250);
    assert_eq!(p.top_causes.len(), 3);
    assert_eq!(p.top_causes[0].reason_code, "VAL01");
    assert_eq!(
        p.top_causes[0].suggested_check.as_deref(),
        Some("EMIR.COMP.NOTIONAL_CURRENCY_MISSING")
    );
    assert_eq!(p.repeated_rejected_utis.len(), 2);
    assert_eq!(p.repeated_rejected_utis[0].uti, "UTI-CHRONIC-001");
}

#[test]
fn fixture_triggers_both_psc_checks() {
    let text = std::fs::read_to_string(fixture_path()).expect("read fixture");
    let profile = serde_yaml::from_str::<RejectionProfileFile>(&text)
        .unwrap()
        .profile;

    // 3 records:
    // - U1: clean (no PSC fires)
    // - UTI-CHRONIC-001: hits REPEATED_REJECTION (count=7 in profile)
    // - U3: notional with missing currency → hits LIKELY_REJECTION_PATTERN
    let records = vec![
        EmirRecord {
            uti: Some("U1".into()),
            record_id: Some("ok".into()),
            notional_amount: Some(Decimal::from(1000)),
            notional_currency: Some("EUR".into()),
            counterparty_1: Some("LEI-OK-001".into()),
            ..Default::default()
        },
        EmirRecord {
            uti: Some("UTI-CHRONIC-001".into()),
            record_id: Some("chronic".into()),
            notional_amount: Some(Decimal::from(2000)),
            notional_currency: Some("USD".into()),
            counterparty_1: Some("LEI-OK-001".into()),
            ..Default::default()
        },
        EmirRecord {
            uti: Some("U3".into()),
            record_id: Some("missing-ccy".into()),
            notional_amount: Some(Decimal::from(3000)),
            notional_currency: None,
            counterparty_1: Some("LEI-OK-001".into()),
            ..Default::default()
        },
    ];

    let ctx = CheckContext::now_with_defaults();
    let issues = run_all_pre_submission(&default_pre_submission_checks(), &records, &profile, &ctx);

    // REPEATED_REJECTION fires once for UTI-CHRONIC-001.
    let repeated: Vec<_> = issues
        .iter()
        .filter(|i| i.check_id == "EMIR.PSC.REPEATED_REJECTION")
        .collect();
    assert_eq!(repeated.len(), 1);
    assert_eq!(repeated[0].uti.as_deref(), Some("UTI-CHRONIC-001"));
    assert!(!repeated[0].evidence.is_empty());

    // LIKELY_REJECTION_PATTERN fires at least once on U3 (matches
    // NOTIONAL_CURRENCY_MISSING which is the top cause).
    let likely: Vec<_> = issues
        .iter()
        .filter(|i| i.check_id == "EMIR.PSC.LIKELY_REJECTION_PATTERN")
        .collect();
    assert!(
        likely
            .iter()
            .any(|i| i.record_id.as_deref() == Some("missing-ccy")),
        "expected LIKELY_REJECTION_PATTERN on the missing-ccy record"
    );

    // U1 (clean) does not appear in any PSC issue.
    assert!(issues.iter().all(|i| i.record_id.as_deref() != Some("ok")));
}
