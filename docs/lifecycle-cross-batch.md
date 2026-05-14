# Cross-batch lifecycle for TSR / MAR / MSR

The single-batch TSR, MAR, MSR layers (`opendqi emir
tr-state-scan` / `mar-scan` / `msr-scan` and the SFTR siblings) each
emit issues based only on the records in the file being scanned. The
**cross-batch lifecycle** layer fires when:

1. the runner is invoked with `--store <path>`, and
2. the store already contains at least one earlier snapshot of the
   same record type.

When both conditions hold, the runner loads the **most recent prior
snapshot** from the store and runs an additional set of `*.LFC.*`
checks comparing current vs. prior. The original snapshot is also
persisted, so the next scan can in turn compare against this one.

Cross-batch lifecycle is opt-in. Without `--store`, OpenDQI runs
entirely in-memory and never touches the database.

## EMIR TSR — 4 `EMIR.TST.LFC.*` checks

| Check ID | Sev | Trigger |
|---|---|---|
| `EMIR.TST.LFC.UTI_DROPPED_WITHOUT_TERMINATION` | High | UTI was outstanding in the prior TSR but is absent from the current snapshot. Verify a TERM lifecycle event was reported in between. |
| `EMIR.TST.LFC.VALUATION_REGRESSION` | Warning | `valuation_amount` drops by > 50% between two snapshots. |
| `EMIR.TST.LFC.MATURITY_CHANGED` | High | Same UTI's `maturity_date` differs between snapshots. |
| `EMIR.TST.LFC.COLLATERAL_PORTFOLIO_CHANGED` | Warning | Same UTI's `collateral_portfolio_code` reassigned (case-insensitive). |

## SFTR TSR — 3 `SFTR.TST.LFC.*` checks

| Check ID | Sev | Trigger |
|---|---|---|
| `SFTR.TST.LFC.UTI_DROPPED_WITHOUT_TERMINATION` | High | Symmetric to the EMIR check, on `SftrTrStateRecord`. |
| `SFTR.TST.LFC.COLLATERAL_VALUE_REGRESSION` | Warning | `collateral_value` drops by > 50% between snapshots. |
| `SFTR.TST.LFC.HAIRCUT_CHANGED` | Warning | Same UTI's `haircut` differs — surfaces collateral re-pricing or correction. |

## EMIR MAR — 3 `EMIR.MAR.LFC.*` checks

| Check ID | Sev | Trigger |
|---|---|---|
| `EMIR.MAR.LFC.PORTFOLIO_GAP` | Warning | Portfolio_code present in the prior batch has no margin event in the current batch. Possible reporting omission. |
| `EMIR.MAR.LFC.RECURRING_LATE_MARGIN` | Warning | Same portfolio shows late reporting in both prior and current batches (recurrent breach of the timeliness threshold). |
| `EMIR.MAR.LFC.NEGATIVE_TREND` | Warning | Maximum `initial_margin_posted` for a portfolio is strictly lower in the current batch than in the prior — sustained de-margining. v1 compares two snapshots; future work will extend to ≥ 3 monotone-decreasing points using scan_id ordering. |

## EMIR MSR — 3 `EMIR.MSR.LFC.*` checks

| Check ID | Sev | Trigger |
|---|---|---|
| `EMIR.MSR.LFC.COLLATERAL_MARKET_VALUE_REGRESSION` | Warning | `collateral_market_value` drops by > 30% between snapshots. |
| `EMIR.MSR.LFC.CATEGORY_CHANGED` | Warning | `collateralization_category` differs between snapshots — e.g. FCOL → PCOL is suspect without a corresponding MAR event. |
| `EMIR.MSR.LFC.HAIRCUT_DRIFT` | Warning | `haircut_applied` changes by > 50% (relative) between snapshots. |

## Store integration

The runners use the new helpers in `crates/opendqi-store/src/load.rs`:

- `load_latest_prior_tr_state(exclude_scan_id)` — returns every row
  from the most recent EMIR TSR scan whose `scan_id < exclude_scan_id`.
- `load_latest_prior_sftr_tr_state(exclude_scan_id)`
- `load_latest_prior_margin_activity(exclude_scan_id)`
- `load_latest_prior_margin_state(exclude_scan_id)`

These are deliberately distinct from the existing per-UTI / per-
portfolio loaders. The latter answer "what do we know about *these*
UTIs from any prior scan?". The lifecycle pass needs a different
question: "what was the *previous full snapshot*?". UTI-dropped /
portfolio-gap detection require the broader view.

## Why a 4th trait?

Three traits were enough until now (`Check`, `LifecycleCheck`, plus
`TrStateCheck` / `SftrTrStateCheck` / `MarginActivityCheck` /
`MarginStateCheck` that share the same shape). G3 introduces:

- `TrStateLifecycleCheck` (EMIR TSR cross-batch)
- `SftrTrStateLifecycleCheck` (SFTR TSR cross-batch)
- `MarginStateLifecycleCheck` (EMIR MSR cross-batch)

…and re-uses the existing `MarginActivityCheck` for the 3 MAR LFC
checks (it already takes `prior: &[MarginActivityRecord]` — the
runners simply pass the latest-prior batch now).

The separation keeps signatures honest: each lifecycle check's
`run` takes `current` and `prior` of the same record type.

## End-to-end verification

```bash
DB=$(mktemp)
# Snapshot 1
opendqi emir tr-state-scan ./snapshot-day-1.xml --store $DB --out ./r1/
# Snapshot 2 (one day later)
opendqi emir tr-state-scan ./snapshot-day-2.xml --store $DB --out ./r2/
grep 'EMIR.TST.LFC' ./r2/tr_state_issues.csv | cut -d, -f1 | sort -u
```

The first run sees no prior snapshot in the store and emits no LFC
issues. The second run loads scan 1 as the prior snapshot and emits
`*.LFC.*` issues for any drift.
