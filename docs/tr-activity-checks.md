# Trade Activity Report (TAR) checks

OpenDQI's TAR layer ingests **activity-oriented** reports — the
list of trade events (NEWT / MODI / CORR / ETRM / VALU / MARU /
POSC / OTHR) that flowed through the Trade Repository during a
period. EMIR uses ISO 20022 `auth.030.001.03`; the same schema
applies whether the file is a firm submission or a TR replay.

The TAR layer answers **"how did the state change?"** in the
three-layer product model (Activity / State / Rejection — see
[`positioning.md`](positioning.md)). It is the complement of the
TSR layer (which answers *"what does the TR believe now?"*) and the
Rejection layer (*"what failed and why?"*).

The TAR layer is invoked via:

```bash
opendqi emir tr-activity-scan <auth.030.xml> [--store <db>] [--tsr <auth.107.xml>] --out <dir>
```

`--tsr` is optional but unlocks the TAR↔TSR coherence check. Outputs:

- `summary.json` — the regime-uniform scan summary.
- `tr_activity_summary.json` — TAR-specific distributions
  (`action_distribution`, `event_distribution`, `total_records`).
- `tr_activity_issues.csv` — flat issue list.
- `tr_activity_report.html` — human-readable report.

## Catalog

| Check ID | Dimension | Severity | What it detects |
|---|---|---|---|
| `EMIR.TRA.REPEATED_CORRECTION` | Accuracy | Warning | Same UTI carries 3 or more `CORR` / `MODI` rows in the batch. Strong signal of an upstream data-quality issue. |
| `EMIR.TRA.SPIKE_TERM` | Accuracy | High | Proportion of `ETRM` / `TERM` actions exceeds 25% of the batch. Unusual termination wave. |
| `EMIR.TRA.SPIKE_MODI` | Accuracy | Warning | Proportion of `MODI` actions exceeds 40% of the batch. Possible over-correction. |
| `EMIR.TRA.DUPLICATE_NEWT_IN_BATCH` | Uniqueness | Critical | Same UTI carries 2 or more `NEWT` rows in the same batch — never legitimate. |
| `EMIR.TRA.NEWT_NOT_IN_TSR` | Consistency | High | UTI is `NEWT`'d in the TAR batch but absent from the companion TSR. Triggers only when `--tsr` is provided. |

## Thresholds

The percentage thresholds for `SPIKE_TERM` (25%) and `SPIKE_MODI`
(40%) are currently compiled-in constants — they will move to the
`Thresholds` YAML config when a real customer profile emerges. The
`REPEATED_CORRECTION` threshold (3 corrections) and the
`DUPLICATE_NEWT_IN_BATCH` threshold (2 NEWTs) are also constants.

## Activity distributions

The `tr_activity_summary.json` sidecar always contains:

```json
{
  "total_records": 20,
  "action_distribution": { "NEWT": 2, "MODI": 9, "CORR": 3, "ETRM": 6 },
  "event_distribution":  { "(missing)": 20 }
}
```

These histograms are the basis of the report's distribution
tables and inform whether the spike checks are likely to fire on
adjacent batches.

## Adding a TAR check

TAR checks live under `crates/opendqi-core/src/dq/tr_activity/`.
Each implements the `TrActivityCheck` trait (signature `(records,
prior, tsr, ctx) -> Vec<DqIssue>`), lives in a single file, ships
two unit tests, and is registered in `default_tr_activity_checks()`
in `crates/opendqi-core/src/dq/mod.rs`.
