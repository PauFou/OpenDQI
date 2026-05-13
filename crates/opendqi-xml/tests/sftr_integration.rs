//! End-to-end test against the SFTR auth.052 synthetic fixture.

use std::path::PathBuf;

use opendqi_xml::read_sftr_xml;

fn fixture_path() -> PathBuf {
    let crate_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(crate_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join("examples/sftr/iso20022/sample.xml")
}

#[test]
fn fixture_extracts_ten_records() {
    let outcome = read_sftr_xml(&fixture_path()).expect("parse");
    assert_eq!(
        outcome.records.len(),
        10,
        "expected 10 records, got {}",
        outcome.records.len()
    );
}

#[test]
fn sft_types_cover_repo_bsb_sleb() {
    let outcome = read_sftr_xml(&fixture_path()).expect("parse");
    let mut sft_types: Vec<&str> = outcome
        .records
        .iter()
        .filter_map(|r| r.sft_type.as_deref())
        .collect();
    sft_types.sort();
    sft_types.dedup();
    for code in ["REPO", "BSB", "SLEB"] {
        assert!(
            sft_types.contains(&code),
            "expected {code} in {sft_types:?}",
        );
    }
}

#[test]
fn clean_record_carries_master_agreement_and_collateral() {
    let outcome = read_sftr_xml(&fixture_path()).expect("parse");
    let clean = outcome
        .records
        .iter()
        .find(|r| r.uti.as_deref() == Some("OPENDQI-SFTR-CLEAN-0001"))
        .expect("clean record");
    assert_eq!(clean.master_agreement_type.as_deref(), Some("GMRA"));
    assert_eq!(clean.master_agreement_version.as_deref(), Some("2011"));
    assert_eq!(clean.loan_currency.as_deref(), Some("EUR"));
    assert!(clean.loan_value.is_some());
    assert!(clean.collateral_value.is_some());
    assert!(clean.haircut.is_some());
    assert_eq!(clean.reuse_indicator, Some(false));
    assert_eq!(clean.collateral_isin.as_deref(), Some("DE0001135275"));
}

#[test]
fn modification_record_carries_prior_uti() {
    let outcome = read_sftr_xml(&fixture_path()).expect("parse");
    let modi = outcome
        .records
        .iter()
        .find(|r| r.action_type.as_deref() == Some("MODI"))
        .expect("MODI record");
    assert_eq!(modi.prior_uti.as_deref(), Some("OPENDQI-SFTR-DUPE-0006"));
}

#[test]
fn fixture_does_not_emit_unsupported_namespace() {
    let outcome = read_sftr_xml(&fixture_path()).expect("parse");
    assert!(
        outcome
            .issues
            .iter()
            .all(|i| i.check_id != "SFTR.FMT.XML_UNSUPPORTED_NAMESPACE"),
        "got namespace warning: {:?}",
        outcome.issues
    );
}
