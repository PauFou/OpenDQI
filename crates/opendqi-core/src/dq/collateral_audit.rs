//! Cross-layer collateral audit — EMIR Article 11 collateral
//! obligation, expressed as a join between the **TSR** (auth.107,
//! trade state) and the **MSR** (auth.109, margin state). Mirrors
//! the structure of [`super::tr_audit`]: a pure free function that
//! consumes records from two parsed messages and emits cross-layer
//! issues; the CLI and (later) the web UI wire it up.
//!
//! Two checks, both new with their own prefix (`EMIR.COL.*`, cousin
//! of `EMIR.AUD.*`):
//!
//! - `EMIR.COL.MISSING` (Completeness / **High**) — for each
//!   outstanding TSR derivative with a non-empty UTI, either:
//!     * no MSR row joinable by UTI ("no link"), or
//!     * matching MSR row(s) where every one of the four
//!       `initial_margin_posted_current` /
//!       `initial_margin_collected_current` /
//!       `variation_margin_posted_current` /
//!       `variation_margin_collected_current` is absent **or** zero
//!       across all matching rows ("all-zero").
//! - `EMIR.COL.STALE` (Timeliness / **Warning**) — when the link
//!   exists and at least one margin amount is non-zero, the most-recent
//!   matching MSR row's `state_as_of` is older than the companion TSR
//!   `state_as_of` (fallback: `now`) by more than
//!   `thresholds.emir_rmt.collateral_max_age_days` (default 1).
//!
//! Honest scoping caveat: `TrStateRecord` (auth.107) does not carry
//! a clearing-status flag, so the check applies to every outstanding
//! TSR derivative. It is therefore a data-quality signal — "TSR
//! outstanding derivative without linkable / fresh MSR margin state" —
//! not a verdict of non-compliance with the Article 11 non-cleared
//! collateral obligation. (Same product discipline as the rest of
//! OpenDQI: data quality, not certification.)

use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::config::Thresholds;
use crate::dq::tr_state::is_outstanding;
use crate::model::{DqDimension, DqIssue, MarginStateRecord, Regime, Severity, TrStateRecord};

const CHECK_MISSING: &str = "EMIR.COL.MISSING";
const CHECK_STALE: &str = "EMIR.COL.STALE";

fn trimmed_nonempty(s: Option<&str>) -> Option<&str> {
    s.map(str::trim).filter(|u| !u.is_empty())
}

/// `true` iff the amount is `Some(d)` and not equal to `Decimal::ZERO`.
fn is_nonzero(d: Option<Decimal>) -> bool {
    match d {
        Some(x) => x != Decimal::ZERO,
        None => false,
    }
}

/// `true` iff at least one of the four IM/VM (posted/collected)
/// current amounts is `Some(d)` and non-zero on this MSR row.
fn row_has_any_margin(m: &MarginStateRecord) -> bool {
    is_nonzero(m.initial_margin_posted_current)
        || is_nonzero(m.initial_margin_collected_current)
        || is_nonzero(m.variation_margin_posted_current)
        || is_nonzero(m.variation_margin_collected_current)
}

/// Render an `Option<Decimal>` for the issue `value` / evidence
/// fields. `None` → `"absent"`, `Some(d)` → `d.to_string()`.
fn fmt_dec(d: Option<Decimal>) -> String {
    match d {
        Some(x) => x.to_string(),
        None => "absent".into(),
    }
}

/// Reference TSR snapshot timestamp: any TSR record's `state_as_of`
/// (they share the report header), else `now` as a conservative
/// fallback when the header is absent in the synthetic/edge case.
fn tsr_reference_state_as_of(tsr: &[TrStateRecord], now: DateTime<Utc>) -> DateTime<Utc> {
    tsr.iter()
        .filter_map(|r| r.state_as_of)
        .max()
        .unwrap_or(now)
}

/// Compute the EMIR collateral cross-layer issues for one (TSR, MSR)
/// pair. See module docs for the exact rules.
pub fn compute_collateral_emir_issues(
    tsr: &[TrStateRecord],
    msr: &[MarginStateRecord],
    thresholds: &Thresholds,
    now: DateTime<Utc>,
) -> Vec<DqIssue> {
    // Index MSR by UTI (trim, non-empty). One UTI may carry several
    // MSR rows (e.g. split by portfolio); we keep them all.
    let mut by_uti: HashMap<&str, Vec<&MarginStateRecord>> = HashMap::new();
    for m in msr {
        if let Some(u) = trimmed_nonempty(m.uti.as_deref()) {
            by_uti.entry(u).or_default().push(m);
        }
    }

    let tsr_ref = tsr_reference_state_as_of(tsr, now);
    let max_age_days = thresholds.emir_rmt.collateral_max_age_days;

    let mut out = Vec::new();
    for r in tsr {
        if !is_outstanding(r) {
            continue;
        }
        let uti = match trimmed_nonempty(r.uti.as_deref()) {
            Some(u) => u,
            // Cannot cross-reference without a UTI; the missing-UTI
            // case is already covered by single-message TSR checks
            // (e.g. `EMIR.TST.OUTSTANDING_SUMMARY`) — don't double-flag.
            None => continue,
        };

        let matches = by_uti.get(uti);

        // (a) No link.
        let Some(rows) = matches else {
            out.push(DqIssue {
                check_id: CHECK_MISSING.into(),
                regime: Regime::Emir,
                severity: Severity::High,
                dimension: DqDimension::Completeness,
                record_id: r.record_id.clone(),
                uti: Some(uti.to_owned()),
                field: Some("margin".into()),
                value: None,
                message: format!(
                    "TSR outstanding derivative {uti} has no linkable MSR margin state \
                     (EMIR Art.11 collateral obligation).",
                ),
                source_file: r.source_file.clone(),
                evidence: Vec::new(),
            });
            continue;
        };

        // (b) Linked but every one of the four IM/VM amounts is
        //     absent-or-zero across *every* matching row.
        if !rows.iter().any(|m| row_has_any_margin(m)) {
            // Snapshot the four amounts from the most-informative row
            // (the first match) so the issue carries concrete
            // evidence of "all zero / absent".
            let m = rows[0];
            let evidence = vec![
                crate::model::EvidenceItem {
                    field: "initial_margin_posted_current".into(),
                    before: None,
                    after: Some(fmt_dec(m.initial_margin_posted_current)),
                    source_line: None,
                },
                crate::model::EvidenceItem {
                    field: "initial_margin_collected_current".into(),
                    before: None,
                    after: Some(fmt_dec(m.initial_margin_collected_current)),
                    source_line: None,
                },
                crate::model::EvidenceItem {
                    field: "variation_margin_posted_current".into(),
                    before: None,
                    after: Some(fmt_dec(m.variation_margin_posted_current)),
                    source_line: None,
                },
                crate::model::EvidenceItem {
                    field: "variation_margin_collected_current".into(),
                    before: None,
                    after: Some(fmt_dec(m.variation_margin_collected_current)),
                    source_line: None,
                },
            ];
            out.push(DqIssue {
                check_id: CHECK_MISSING.into(),
                regime: Regime::Emir,
                severity: Severity::High,
                dimension: DqDimension::Completeness,
                record_id: r.record_id.clone(),
                uti: Some(uti.to_owned()),
                field: Some("margin".into()),
                value: None,
                message: format!(
                    "TSR outstanding derivative {uti} has an MSR margin row, but every IM/VM \
                     (posted/collected) amount is absent or zero \
                     (EMIR Art.11 collateral obligation).",
                ),
                source_file: r.source_file.clone(),
                evidence,
            });
            continue;
        }

        // (c) Linked, has some non-zero margin → staleness check.
        //     Take the most-recent MSR `state_as_of` for this UTI.
        let msr_latest: Option<DateTime<Utc>> = rows.iter().filter_map(|m| m.state_as_of).max();
        let Some(msr_ts) = msr_latest else {
            // Non-stale fingerprint: we have margin but no snapshot
            // timestamp anywhere — can't assess staleness, don't flag
            // (consistent with the data-quality discipline of not
            // over-reporting on missing metadata).
            continue;
        };
        let age_days = (tsr_ref - msr_ts).num_days();
        if age_days > max_age_days {
            out.push(DqIssue {
                check_id: CHECK_STALE.into(),
                regime: Regime::Emir,
                severity: Severity::Warning,
                dimension: DqDimension::Timeliness,
                record_id: r.record_id.clone(),
                uti: Some(uti.to_owned()),
                field: Some("state_as_of".into()),
                value: Some(msr_ts.to_rfc3339()),
                message: format!(
                    "MSR margin state for outstanding TSR derivative {uti} is {age_days} \
                     calendar day(s) older than the TSR snapshot (threshold: \
                     {max_age_days}; EMIR Art.11 daily margining).",
                ),
                source_file: r.source_file.clone(),
                evidence: vec![crate::model::EvidenceItem {
                    field: "state_as_of".into(),
                    before: Some(msr_ts.to_rfc3339()),
                    after: Some(tsr_ref.to_rfc3339()),
                    source_line: None,
                }],
            });
        }
    }

    // Stable, deterministic order: by check-id then UTI. The downstream
    // `finalize_issues` re-imposes the canonical content total-order,
    // so this is mainly for test/debug readability.
    out.sort_by(|a, b| a.check_id.cmp(&b.check_id).then_with(|| a.uti.cmp(&b.uti)));
    out
}

// Silence unused-import lint when no callers use `BTreeMap` here.
#[allow(dead_code)]
type _BTreeMapAlias = BTreeMap<String, String>;

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    fn ts(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    fn now() -> DateTime<Utc> {
        ts("2026-05-19T08:00:00Z")
    }

    fn tsr(uti: &str, status: &str) -> TrStateRecord {
        TrStateRecord {
            source_file: Some("tsr.xml".into()),
            record_id: Some(uti.into()),
            regime: Regime::Emir,
            state_as_of: Some(ts("2026-05-19T00:00:00Z")),
            uti: Some(uti.into()),
            status: Some(status.into()),
            ..Default::default()
        }
    }

    fn msr(uti: &str, state: DateTime<Utc>) -> MarginStateRecord {
        MarginStateRecord {
            source_file: Some("msr.xml".into()),
            record_id: Some(uti.into()),
            regime: Regime::Emir,
            uti: Some(uti.into()),
            state_as_of: Some(state),
            ..Default::default()
        }
    }

    fn with_margin(mut m: MarginStateRecord, vm_posted: Decimal) -> MarginStateRecord {
        m.variation_margin_posted_current = Some(vm_posted);
        m
    }

    fn t() -> Thresholds {
        Thresholds::default()
    }

    #[test]
    fn no_link_flags_missing() {
        let tsr_rows = vec![tsr("U1", "OUTSTANDING")];
        let msr_rows: Vec<MarginStateRecord> = vec![];
        let issues = compute_collateral_emir_issues(&tsr_rows, &msr_rows, &t(), now());
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].check_id, "EMIR.COL.MISSING");
        assert_eq!(issues[0].uti.as_deref(), Some("U1"));
        assert!(
            issues[0].evidence.is_empty(),
            "no-link issue carries no evidence"
        );
    }

    #[test]
    fn linked_all_zero_flags_missing_with_evidence() {
        let tsr_rows = vec![tsr("U1", "OUTSTANDING")];
        let mut m = msr("U1", ts("2026-05-19T00:00:00Z"));
        m.initial_margin_posted_current = Some(Decimal::ZERO);
        m.variation_margin_collected_current = Some(Decimal::ZERO);
        let issues = compute_collateral_emir_issues(&tsr_rows, &[m], &t(), now());
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].check_id, "EMIR.COL.MISSING");
        assert_eq!(
            issues[0].evidence.len(),
            4,
            "all four IM/VM fields recorded"
        );
    }

    #[test]
    fn linked_nonzero_fresh_no_issue() {
        let tsr_rows = vec![tsr("U1", "OUTSTANDING")];
        let m = with_margin(msr("U1", ts("2026-05-19T00:00:00Z")), Decimal::new(1000, 0));
        let issues = compute_collateral_emir_issues(&tsr_rows, &[m], &t(), now());
        assert!(issues.is_empty(), "fresh, linked, non-zero margin → clean");
    }

    #[test]
    fn linked_nonzero_stale_flags_stale() {
        let tsr_rows = vec![tsr("U1", "OUTSTANDING")]; // tsr state_as_of = 2026-05-19
        let m = with_margin(msr("U1", ts("2026-05-15T00:00:00Z")), Decimal::new(500, 0));
        let issues = compute_collateral_emir_issues(&tsr_rows, &[m], &t(), now());
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].check_id, "EMIR.COL.STALE");
        assert_eq!(issues[0].evidence.len(), 1);
    }

    #[test]
    fn matured_trade_skipped() {
        let tsr_rows = vec![tsr("U1", "MATURED")];
        let issues = compute_collateral_emir_issues(&tsr_rows, &[], &t(), now());
        assert!(issues.is_empty(), "non-outstanding TSR rows are skipped");
    }

    #[test]
    fn empty_uti_skipped_no_double_flag() {
        let mut r = tsr("ignored", "OUTSTANDING");
        r.uti = Some("   ".into()); // whitespace-only
        let issues = compute_collateral_emir_issues(&[r], &[], &t(), now());
        assert!(issues.is_empty(), "empty UTI cannot be cross-referenced");
    }

    #[test]
    fn multiple_msr_rows_some_nonzero_no_missing() {
        // Two MSR rows for the same UTI: one all-zero, one non-zero.
        // Aggregate rule says "any non-zero across matches" ⇒ clean.
        let tsr_rows = vec![tsr("U1", "OUTSTANDING")];
        let mut zero_row = msr("U1", ts("2026-05-19T00:00:00Z"));
        zero_row.initial_margin_posted_current = Some(Decimal::ZERO);
        let live_row = with_margin(msr("U1", ts("2026-05-19T01:00:00Z")), Decimal::new(750, 0));
        let issues = compute_collateral_emir_issues(&tsr_rows, &[zero_row, live_row], &t(), now());
        assert!(
            issues.is_empty(),
            "if ANY matching MSR row has margin, the trade isn't MISSING"
        );
    }

    #[test]
    fn tsr_without_state_as_of_falls_back_to_now() {
        // No `state_as_of` on the TSR ⇒ staleness measured vs `now`.
        let mut r = tsr("U1", "OUTSTANDING");
        r.state_as_of = None;
        // 5 days old vs now(2026-05-19) ⇒ stale (threshold default = 1).
        let m = with_margin(msr("U1", ts("2026-05-14T08:00:00Z")), Decimal::new(100, 0));
        let issues = compute_collateral_emir_issues(&[r], &[m], &t(), now());
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].check_id, "EMIR.COL.STALE");
    }
}
