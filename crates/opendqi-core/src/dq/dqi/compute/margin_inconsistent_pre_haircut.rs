//! `DQI_MARGIN_INCONSISTENT_PRE_HAIRCUT` — share of per-trade
//! reconciliation records where the firm's IM/VM amount
//! (pre-haircut) disagrees with the counterparty's, as
//! reported in `auth.091` per-tx detail.
//!
//! - **Layer:** auth.091 per-tx → [`crate::ReconciliationRecord`].
//! - **Denominator:** records with `pairing_status` populated.
//! - **Numerator:** records whose `mismatched_fields` contains
//!   any of [`PRE_HAIRCUT_TOKENS`].
//! - **Dimension:** consistency.

use crate::dq::dqi::compute::criterion_mismatch_rate;
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, ReconciliationRecord};
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_MARGIN_INCONSISTENT_PRE_HAIRCUT";
const DESCRIPTION: &str = "Share of paired auth.091 records where the IM/VM posted-or-received \
PRE-haircut amount mismatched between counterparties. Token set defaults from canonical \
ESMA EMIR REFIT auth.091 ValtnMtchgCrit leaf names.";

/// Canonical ESMA EMIR REFIT auth.091 `MtchgCrit` leaf names
/// for **pre-haircut** initial / variation margin amounts.
pub const PRE_HAIRCUT_TOKENS: &[&str] = &[
    "InitlMrgnPstdPreHrcut",
    "InitlMrgnRcvdPreHrcut",
    "VartnMrgnPstdPreHrcut",
    "VartnMrgnRcvdPreHrcut",
];

/// Compute `DQI_MARGIN_INCONSISTENT_PRE_HAIRCUT`.
pub fn compute_dqi_margin_inconsistent_pre_haircut(
    recon_records: &[ReconciliationRecord],
    thresholds: &Thresholds,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    criterion_mismatch_rate::compute(
        recon_records,
        INDICATOR_ID,
        DESCRIPTION,
        PRE_HAIRCUT_TOKENS,
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
        let (ind, _) = compute_dqi_margin_inconsistent_pre_haircut(
            &[],
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn im_pre_haircut_token_fires() {
        let recs = vec![rec("U1", vec!["InitlMrgnPstdPreHrcut"]), rec("U2", vec![])];
        let (ind, _) = compute_dqi_margin_inconsistent_pre_haircut(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 2);
        assert_eq!(ind.numerator, 1);
    }

    #[test]
    fn post_haircut_token_does_not_fire() {
        // PRE-haircut DQI must NOT trip on POST-haircut mismatches.
        let recs = vec![rec("U1", vec!["InitlMrgnPstdPstHrcut"])];
        let (ind, _) = compute_dqi_margin_inconsistent_pre_haircut(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 0);
    }

    #[test]
    fn vm_pre_haircut_token_fires() {
        let recs = vec![rec("U1", vec!["VartnMrgnRcvdPreHrcut"])];
        let (ind, _) = compute_dqi_margin_inconsistent_pre_haircut(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 1);
    }
}
