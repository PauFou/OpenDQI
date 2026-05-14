//! End-to-end test of the SFTR rejection-profile → pre-submission
//! loop. Mirrors the EMIR variant.

use std::path::PathBuf;

use opendqi_core::dq::{
    default_sftr_pre_submission_checks, run_all_sftr_pre_submission, CheckContext,
};
use opendqi_core::{RejectionProfileFile, SftrRecord};
use rust_decimal::Decimal;

fn fixture_path() -> PathBuf {
    let crate_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(crate_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join("examples/sftr/rejection_profile/sample.yml")
}

#[test]
fn fixture_deserialises_into_sftr_rejection_profile() {
    let text = std::fs::read_to_string(fixture_path()).expect("read fixture");
    let file: RejectionProfileFile = serde_yaml::from_str(&text).expect("parse profile");
    let p = file.profile;
    assert_eq!(p.total_feedbacks, 200);
    assert_eq!(p.top_causes.len(), 3);
    assert_eq!(p.top_causes[0].reason_code, "SFTRVAL02");
    assert_eq!(
        p.top_causes[0].suggested_check.as_deref(),
        Some("SFTR.COMP.COLLATERAL_VALUE_MISSING")
    );
    assert_eq!(p.repeated_rejected_utis.len(), 2);
    assert_eq!(p.repeated_rejected_utis[0].uti, "SFT-CHRONIC-001");
}

#[test]
fn fixture_triggers_both_sftr_psc_checks() {
    let text = std::fs::read_to_string(fixture_path()).expect("read fixture");
    let profile = serde_yaml::from_str::<RejectionProfileFile>(&text)
        .unwrap()
        .profile;

    let records = vec![
        SftrRecord {
            uti: Some("U1".into()),
            record_id: Some("ok".into()),
            collateral_value: Some(Decimal::from(1000)),
            ..Default::default()
        },
        SftrRecord {
            uti: Some("SFT-CHRONIC-001".into()),
            record_id: Some("chronic".into()),
            collateral_value: Some(Decimal::from(2000)),
            ..Default::default()
        },
        SftrRecord {
            uti: Some("U3".into()),
            record_id: Some("missing-coll".into()),
            collateral_value: None,
            ..Default::default()
        },
    ];

    let ctx = CheckContext::now_with_defaults();
    let issues = run_all_sftr_pre_submission(
        &default_sftr_pre_submission_checks(),
        &records,
        &profile,
        &ctx,
    );

    // REPEATED_REJECTION fires once for SFT-CHRONIC-001.
    let repeated: Vec<_> = issues
        .iter()
        .filter(|i| i.check_id == "SFTR.PSC.REPEATED_REJECTION")
        .collect();
    assert_eq!(repeated.len(), 1);
    assert_eq!(repeated[0].uti.as_deref(), Some("SFT-CHRONIC-001"));
    assert!(!repeated[0].evidence.is_empty());

    // LIKELY_REJECTION_PATTERN fires on the missing-collateral record.
    let likely: Vec<_> = issues
        .iter()
        .filter(|i| i.check_id == "SFTR.PSC.LIKELY_REJECTION_PATTERN")
        .collect();
    assert!(likely
        .iter()
        .any(|i| i.record_id.as_deref() == Some("missing-coll")));

    // U1 (clean) does not appear in any PSC issue.
    assert!(issues.iter().all(|i| i.record_id.as_deref() != Some("ok")));
}
