//! `DQI_NOTIONAL_INCONSISTENT` — share of per-trade
//! reconciliation records where the firm's notional amount
//! disagrees with the counterparty's notional, as reported in
//! the `auth.091` per-tx detail (`MtchgCrit` block).
//!
//! - **Layer:** auth.091 per-tx reconciliation records →
//!   [`crate::ReconciliationRecord`].
//! - **Denominator:** records with `pairing_status` populated
//!   (so the criterion comparison is meaningful — UNPAIRED
//!   records contribute trivially).
//! - **Numerator:** records whose `mismatched_fields` contains
//!   any of the ESMA "notional" criterion tokens — see
//!   [`NOTIONAL_TOKENS`].
//! - **Dimension:** consistency.
//!
//! The token list is intentionally a **default** rather than
//! a hardcoded enum so future v0.17 thresholds-config can
//! override it without a code change. For v0.16 we ship the
//! canonical ESMA EMIR REFIT auth.091 leaf names (case-
//! sensitive — the parser projects them as-is from
//! `MtchgCrit/<criterion>` element names).
//!
//! See the per-criterion peers in
//! `margin_inconsistent_pre_haircut.rs` and
//! `margin_inconsistent_post_haircut.rs`.

use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, ReconciliationRecord};
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_NOTIONAL_INCONSISTENT";
const DESCRIPTION: &str = "Share of paired auth.091 records where the notional-amount criterion \
mismatched between counterparties. Token set defaults from canonical ESMA EMIR REFIT auth.091 \
MtchgCrit leaf names ; configurable override via dqi.* YAML block (v0.17+).";

/// Canonical ESMA EMIR REFIT auth.091 `MtchgCrit` leaf names
/// that represent the notional amount. Multiple tokens because
/// notional can be reported under different criterion families
/// (CtrctMtchgCrit / TxMtchgCrit) depending on the trade type.
pub const NOTIONAL_TOKENS: &[&str] = &["NtnlAmt", "NtnlAmtFstLeg", "NtnlAmtScndLeg"];

/// Compute `DQI_NOTIONAL_INCONSISTENT`.
pub fn compute_dqi_notional_inconsistent(
    recon_records: &[ReconciliationRecord],
    thresholds: &Thresholds,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    super::criterion_mismatch_rate::compute(
        recon_records,
        INDICATOR_ID,
        DESCRIPTION,
        NOTIONAL_TOKENS,
        DqDimension::Consistency,
        thresholds,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dq::dqi::DqiStatus;

    fn rec(uti: &str, status: Option<&str>, mismatched: Vec<&str>) -> ReconciliationRecord {
        ReconciliationRecord {
            uti: Some(uti.into()),
            pairing_status: status.map(|s| s.into()),
            mismatched_fields: mismatched.into_iter().map(String::from).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn empty_input_is_not_applicable() {
        let (ind, _) = compute_dqi_notional_inconsistent(
            &[],
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn notional_token_fires() {
        let recs = vec![
            rec("U1", Some("PAIRED"), vec!["NtnlAmt"]),
            rec("U2", Some("PAIRED"), vec![]),
        ];
        let (ind, ev) = compute_dqi_notional_inconsistent(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.denominator, 2);
        assert_eq!(ind.numerator, 1);
        assert_eq!(ev.len(), 1);
        assert_eq!(ev[0].uti, "U1");
    }

    #[test]
    fn non_notional_mismatch_does_not_fire() {
        let recs = vec![rec("U1", Some("PAIRED"), vec!["CtrctTp"])];
        let (ind, _) = compute_dqi_notional_inconsistent(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.denominator, 1);
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn leg_specific_tokens_count() {
        let recs = vec![rec("U1", Some("PAIRED"), vec!["NtnlAmtFstLeg"])];
        let (ind, _) = compute_dqi_notional_inconsistent(
            &recs,
            &Thresholds::default(),
            MappingPresence::default(),
        );
        assert_eq!(ind.numerator, 1);
    }
}
