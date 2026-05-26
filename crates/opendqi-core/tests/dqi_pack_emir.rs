//! Integration test for the EMIR Data Quality Pack public
//! surface (v0.15 D4).
//!
//! Exercises the orchestrator via the **re-exported** names
//! from the crate root (not via `crate::dq::dqi::*`) so that
//! any future internal refactor that accidentally drops a
//! public re-export breaks here, not silently.

use chrono::NaiveDate;
use opendqi_core::{
    compute_emir_dqi_pack, DqiPackResult, DqiStatus, EmirDqiInputs, FeedbackRecord, FeedbackType,
    MappingPresence, Thresholds,
};

fn as_of() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 5, 21).unwrap()
}

#[test]
fn empty_inputs_yield_28_indicators_all_not_applicable() {
    let result: DqiPackResult = compute_emir_dqi_pack(
        EmirDqiInputs::default(),
        MappingPresence::default(),
        &Thresholds::default(),
        as_of(),
    );
    // 28 = 10 (v0.15 baseline) + 7 (v0.16 auth.091 cross-CP) +
    //       4 (v0.16 presence) + 3 (v0.16 final) + 4 (v0.18 EMIR.POS.*).
    // The test was at 24 from v0.16 onward and missed the +4 EMIR.POS.*
    // bump during v0.18's preflight (a parallel empty-inputs test got
    // the update; this one didn't). Bumped here as the v0.19 release
    // hygiene pass — same pattern as v0.18's pre-tag style/fmt fix.
    assert_eq!(result.indicators.len(), 28);
    assert!(result
        .indicators
        .iter()
        .all(|i| i.status == DqiStatus::NotApplicable));
}

#[test]
fn feedback_only_pack_computes_2_indicators() {
    let feedback = vec![
        FeedbackRecord {
            uti: Some("U1".into()),
            feedback_type: FeedbackType::Rejected,
            ..Default::default()
        },
        FeedbackRecord {
            uti: Some("U2".into()),
            feedback_type: FeedbackType::Rejected,
            ..Default::default()
        },
    ];
    let inputs = EmirDqiInputs {
        feedback: Some(&feedback),
        ..Default::default()
    };
    let result = compute_emir_dqi_pack(
        inputs,
        MappingPresence::default(),
        &Thresholds::default(),
        as_of(),
    );
    let computed: Vec<&str> = result
        .indicators
        .iter()
        .filter(|i| i.status != DqiStatus::NotApplicable)
        .map(|i| i.indicator_id.as_str())
        .collect();
    assert_eq!(computed, vec!["DQI_REJ_RATE", "DQI_REJ_REPEAT_UTI"]);
}
