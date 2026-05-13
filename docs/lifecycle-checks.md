# Lifecycle (cross-batch) checks

OpenDQI's "lifecycle" checks compare the records of the current scan
against the history of previously scanned records. They detect
anomalies that no single-batch check can see — a MODI action with no
prior NEWT, a NEWT for a UTI that already has one, a VALU that
regresses to an earlier timestamp than the last known valuation, a
VALU that arrives after the trade has been terminated.

Lifecycle checks are **opt-in**: they only run when you pass
`--store <path>` to `opendqi {emir,sftr} scan`. Without the flag,
OpenDQI's behaviour is exactly what it was before this feature
landed — no SQLite file is opened, no database is touched, no
lifecycle issues are produced.

See [`history-store.md`](history-store.md) for the storage format.

## Catalog

### EMIR (5)

| Check ID | Dimension | Severity | What it detects |
|---|---|---|---|
| `EMIR.LFC.MODI_WITHOUT_NEWT` | Consistency | High | A MODI or CORR action whose UTI has no prior NEWT in the history store. |
| `EMIR.LFC.ETRM_WITHOUT_NEWT` | Consistency | High | An ETRM (early termination) whose UTI has no prior NEWT in the history store. |
| `EMIR.LFC.DUPLICATE_NEWT_FOR_UTI` | Uniqueness | Critical | A NEWT for a UTI that already has a prior NEWT in the history store — a lifecycle duplicate. |
| `EMIR.LFC.VALUATION_REGRESSION` | Consistency | Warning | A VALU whose `valuation_timestamp` is strictly earlier than the latest prior VALU timestamp for the same UTI. |
| `EMIR.LFC.VALUATION_AFTER_TERMINATION` | Consistency | High | A VALU whose `valuation_timestamp.date()` is on or after a prior ETRM's `termination_date` for the same UTI. |

### SFTR (3)

| Check ID | Dimension | Severity | What it detects |
|---|---|---|---|
| `SFTR.LFC.MODI_WITHOUT_NEWT` | Consistency | High | A MODI or CORR action whose UTI has no prior NEWT in the history store. |
| `SFTR.LFC.ETRM_WITHOUT_NEWT` | Consistency | High | An ETRM whose UTI has no prior NEWT in the history store. |
| `SFTR.LFC.DUPLICATE_NEWT_FOR_UTI` | Uniqueness | Critical | A NEWT for a UTI that already has a prior NEWT in the history store. |

SFTR does not currently have `VALUATION_REGRESSION` /
`VALUATION_AFTER_TERMINATION` counterparts: the SFTR record does not
carry a typed valuation timestamp on which a clean lifecycle check
could be built. Collateral value can legitimately fluctuate, so a
naive "value regression" is not by itself a defect.

## Semantics

Each lifecycle check sees two slices:

- **current**: the records loaded from this scan's inputs.
- **prior**: records from earlier scans for the UTIs present in the
  current batch, loaded from the SQLite store.

The current scan is persisted **before** prior is loaded, with the
prior query filtering out `scan_id >= current_scan_id`. This means a
check never sees the current batch in `prior` — only strictly older
scans.

## Algorithmic complexity

Each check runs in `O(N + M)` where `N` is the size of the current
batch and `M` is the size of the prior slice. The prior slice is
already filtered to UTIs present in the current batch, so it is
generally small relative to the total store size.

## Adding a lifecycle check

EMIR checks live under `crates/opendqi-core/src/dq/lifecycle/`,
SFTR checks under `crates/opendqi-core/src/dq/sftr/lifecycle/`.
Implement `LifecycleCheck` (or `SftrLifecycleCheck`), add positive
and negative unit tests, and register the struct in
`default_lifecycle_checks()` / `default_sftr_lifecycle_checks()` in
`crates/opendqi-core/src/dq/mod.rs`.
