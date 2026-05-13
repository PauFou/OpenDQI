# Trade State Report (TSR) checks

OpenDQI's TSR layer ingests the **state-oriented** report a Trade
Repository periodically sends to the firm — its view of which trades
are currently outstanding, with their latest valuation, notional,
collateral portfolio, and maturity. EMIR uses ISO 20022 `auth.107`
(SFTR uses `auth.079`, on the Phase 6 roadmap).

Unlike the activity-oriented TAR or the rejection-oriented feedback
report, a TSR is a **snapshot**. Each line answers "what does the TR
believe today about this UTI?". OpenDQI's seven state-health checks
operate on that snapshot.

The TSR layer is invoked via:

```bash
opendqi emir tr-state-scan <auth.107.xml> [--store <db>] --out <dir>
```

`--store` is optional. Outputs are deliberately distinct from the
activity / feedback layers (`summary.json`, `tr_state_issues.csv`,
`tr_state_report.html`) so they can co-exist in the same `--out`
directory.

See [`auth-messages.md`](auth-messages.md) for the canonical
message catalog and [`positioning.md`](positioning.md) for the
three-layer product model.

## Catalog

| Check ID | Dimension | Severity | What it detects |
|---|---|---|---|
| `EMIR.TST.OUTSTANDING_SUMMARY` | Completeness | Info | One Info issue per outstanding trade — populates the report's outstanding list without polluting the issue count (Info is excluded from the quality score). |
| `EMIR.TST.STALE_VALUATION` | Accuracy | High | The latest valuation timestamp the TR is holding is older than `thresholds.timeliness.max_valuation_age_business_days` relative to the TSR's `state_as_of` (or `ctx.today` fallback). |
| `EMIR.TST.MISSING_VALUATION` | Completeness | High | TR shows an outstanding trade with no valuation amount. |
| `EMIR.TST.ACTIVE_PAST_MATURITY` | Consistency | High | TR shows the trade as outstanding but its maturity date is already in the past (and no termination date is recorded). |
| `EMIR.TST.PLACEHOLDER_MATURITY` | Accuracy | Warning | Maturity date matches one of the configured placeholder dates (default: `1900-01-01`, `2099-12-31`, `9999-12-31`). |
| `EMIR.TST.DUPLICATE_ACTIVE_UTI` | Uniqueness | Critical | Same UTI appears more than once among outstanding trades in the batch. |
| `EMIR.TST.VALUATION_AFTER_TERMINATION` | Consistency | High | TR shows a termination date and a valuation timestamp that is strictly after termination. |

## Semantics & reference clock

Every TSR record carries a `state_as_of` field propagated from the
report header (`<Hdr><StateAsOf>…</StateAsOf></Hdr>`). Time-based
checks use `state_as_of` as their reference clock; when the header
is missing, they fall back to `ctx.now` / `ctx.today` from
[`CheckContext`].

This makes the checks deterministic and reproducible: re-scanning the
same TSR file always produces the same issue list, regardless of when
the scan runs.

## Configuration

The two thresholds that drive the TSR layer live in the existing
`Thresholds` config:

- `timeliness.max_valuation_age_business_days` (default: `1`) — used
  by `STALE_VALUATION`.
- `maturity.placeholder_dates` (default: `[1900-01-01, 2099-12-31,
  9999-12-31]`) — used by `PLACEHOLDER_MATURITY`.

Pass `--config thresholds.yml` to override either subset.

## Adding a TSR check

TSR checks live under `crates/opendqi-core/src/dq/tr_state/`. Each
implements the `TrStateCheck` trait, lives in a single file, ships
two unit tests (positive + negative), and is registered in
`default_tr_state_checks()` in `crates/opendqi-core/src/dq/mod.rs`.
