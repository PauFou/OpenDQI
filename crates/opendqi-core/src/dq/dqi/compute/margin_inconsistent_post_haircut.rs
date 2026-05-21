//! `DQI_MARGIN_INCONSISTENT_POST_HAIRCUT` — share of per-trade
//! reconciliation records where the firm's IM/VM amount
//! (post-haircut) disagrees with the counterparty's.
//!
//! - **Layer:** auth.091 per-tx → [`crate::ReconciliationRecord`].
//! - **Denominator:** records with `pairing_status` populated.
//! - **Numerator:** records whose `mismatched_fields` contains
//!   any of [`POST_HAIRCUT_TOKENS`].
//! - **Dimension:** consistency.

use crate::dq::dqi::compute::criterion_mismatch_rate;
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, ReconciliationRecord};
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_MARGIN_INCONSISTENT_POST_HAIRCUT";
const DESCRIPTION: &str = "Share of paired auth.091 records where the IM/VM posted-or-received \
POST-haircut amount mismatched between counterparties. Token set defaults from canonical \
ESMA EMIR REFIT auth.091 ValtnMtchgCrit leaf names.";

/// Canonical ESMA EMIR REFIT auth.091 `MtchgCrit` leaf names
/// for **post-haircut** initial / variation margin amounts.
pub const POST_HAIRCUT_TOKENS: &[&str] = &[
    "InitlMrgnPstdPstHrcut",
    "InitlMrgnRcvdPstHrcut",
    "VartnMrgnPstdPstHrcut",
    "VartnMrgnRcvdPstHrcut",
];

/// Compute `DQI_MARGIN_INCONSISTENT_POST_HAIRCUT`.
pub fn compute_dqi_margin_inconsistent_post_haircut(
    recon_records: &[ReconciliationRecord],
    thresholds: &Thresholds,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    criterion_mismatch_rate::compute(
        recon_records,
        INDICATOR_ID,
        DESCRIPTION,
        POST_HAIRCUT_TOKENS,
        DqDimension::Consistency,
        thresholds,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dq::dqi::DqiStatus;

    fn rec(uti: &str, mismatched: Vec<&str>) -> ReconciliationRecord {
        ReconciliationRecord {
            uti: Some(uti.into()),
            pairing_status: Some("PAIRED".into()),
            mismatched_fields: mismatched.into_iter().map(String::from).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn empty_input_is_not_applicable() {
        let (ind, _) = compute_dqi_margin_inconsistent_post_haircut(
            &[],
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn im_post_haircut_token_fires() {
        let recs = vec![rec("U1", vec!["InitlMrgnPstdPstHrcut"])];
        let (ind, _) = compute_dqi_margin_inconsistent_post_haircut(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 1);
    }

    #[test]
    fn pre_haircut_token_does_not_fire() {
        // POST-haircut DQI must NOT trip on PRE-haircut mismatches.
        let recs = vec![rec("U1", vec!["InitlMrgnPstdPreHrcut"])];
        let (ind, _) = compute_dqi_margin_inconsistent_post_haircut(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 0);
    }
}
