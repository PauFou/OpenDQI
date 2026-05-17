//! SFTR.MCR.* cross-reference of the auth.083 requested UTIs against a
//! companion SFTR Trade State Report (`auth.079`, supplied via `--tsr`
//! or loaded as the latest per-UTI state from the history store).
//!
//! No-ops entirely when no TSR is available (`tsr` is `None`) — the
//! plain `missing-collateral` scan stays byte-identical. Records with
//! no UTI are skipped (already covered by `MISSING_UTI_ON_REQUEST`).

use std::collections::HashMap;

use super::{build_issue, counterparty_label, MissingCollateralCheck};
use crate::dq::CheckContext;
use crate::model::{DqDimension, DqIssue, MissingCollateralRecord, Severity, SftrTrStateRecord};
use rust_decimal::Decimal;

/// Check implementation.
pub struct SftrMcrCollateralStateCrossRef;

const ID_PRESENT: &str = "SFTR.MCR.COLLATERAL_PRESENT_IN_TSR";
const ID_STILL_MISSING: &str = "SFTR.MCR.STILL_MISSING_IN_TSR";
const ID_NOT_IN_TSR: &str = "SFTR.MCR.REQUESTED_UTI_NOT_IN_TSR";

/// True when the TR state shows collateral for this SFT.
fn has_collateral(s: &SftrTrStateRecord) -> bool {
    s.collateral_value.is_some_and(|v| v > Decimal::ZERO)
        || s.collateral_isin.as_deref().is_some_and(|i| !i.is_empty())
}

impl MissingCollateralCheck for SftrMcrCollateralStateCrossRef {
    fn id(&self) -> &'static str {
        // Representative id (this check emits a small family).
        ID_STILL_MISSING
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Consistency
    }
    fn severity(&self) -> Severity {
        Severity::High
    }
    fn run(
        &self,
        records: &[MissingCollateralRecord],
        tsr: Option<&[SftrTrStateRecord]>,
        _ctx: &CheckContext,
    ) -> Vec<DqIssue> {
        let Some(tsr) = tsr else {
            return Vec::new();
        };
        // Last write wins — a later TSR row supersedes an earlier one
        // for the same UTI.
        let by_uti: HashMap<&str, &SftrTrStateRecord> = tsr
            .iter()
            .filter_map(|s| s.uti.as_deref().map(|u| (u, s)))
            .collect();

        records
            .iter()
            .filter_map(|r| {
                let uti = r.uti.as_deref()?;
                let cp = counterparty_label(r);
                let issue = match by_uti.get(uti) {
                    None => {
                        let mut i =
                            build_issue(ID_NOT_IN_TSR, Severity::High, DqDimension::Consistency, r);
                        i.message = format!(
                            "Requested SFT {uti} ({cp}) is not present in the SFTR \
                             trade-state report — the firm's TR state has no such SFT."
                        );
                        i
                    }
                    Some(s) if has_collateral(s) => {
                        let mut i =
                            build_issue(ID_PRESENT, Severity::Info, DqDimension::Consistency, r);
                        i.message = format!(
                            "Requested SFT {uti} ({cp}) already shows collateral in the \
                             TR trade state — the request is likely satisfied (or TR lag)."
                        );
                        i
                    }
                    Some(_) => {
                        let mut i = build_issue(
                            ID_STILL_MISSING,
                            Severity::High,
                            DqDimension::Consistency,
                            r,
                        );
                        i.message = format!(
                            "Requested SFT {uti} ({cp}) is in the TR trade state but \
                             still has no collateral — the missing-collateral gap is confirmed."
                        );
                        i
                    }
                };
                Some(issue)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tsr(uti: &str, coll: Option<&str>) -> SftrTrStateRecord {
        SftrTrStateRecord {
            uti: Some(uti.into()),
            collateral_value: coll.map(|c| c.parse().unwrap()),
            ..Default::default()
        }
    }

    fn req(uti: Option<&str>) -> MissingCollateralRecord {
        MissingCollateralRecord {
            uti: uti.map(Into::into),
            ..Default::default()
        }
    }

    #[test]
    fn no_tsr_is_a_noop() {
        let recs = vec![req(Some("U1"))];
        let ctx = CheckContext::now_with_defaults();
        assert!(SftrMcrCollateralStateCrossRef
            .run(&recs, None, &ctx)
            .is_empty());
    }

    #[test]
    fn classifies_present_missing_and_absent() {
        let recs = vec![
            req(Some("U-PRESENT")),
            req(Some("U-EMPTY")),
            req(Some("U-ABSENT")),
            req(None), // skipped — covered by MISSING_UTI_ON_REQUEST
        ];
        let state = vec![tsr("U-PRESENT", Some("1000.0")), tsr("U-EMPTY", None)];
        let ctx = CheckContext::now_with_defaults();
        let issues = SftrMcrCollateralStateCrossRef.run(&recs, Some(&state), &ctx);
        assert_eq!(issues.len(), 3);
        let ids: Vec<&str> = issues.iter().map(|i| i.check_id.as_str()).collect();
        assert!(ids.contains(&ID_PRESENT));
        assert!(ids.contains(&ID_STILL_MISSING));
        assert!(ids.contains(&ID_NOT_IN_TSR));
        let present = issues.iter().find(|i| i.check_id == ID_PRESENT).unwrap();
        assert_eq!(present.severity, Severity::Info);
    }

    #[test]
    fn collateral_isin_counts_as_present() {
        let recs = vec![req(Some("U1"))];
        let mut s = tsr("U1", None);
        s.collateral_isin = Some("ISINSYNTHETIC01".into());
        let ctx = CheckContext::now_with_defaults();
        let issues = SftrMcrCollateralStateCrossRef.run(&recs, Some(&[s]), &ctx);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].check_id, ID_PRESENT);
    }
}
