# `examples/sftr-data-quality-pack/` — reproducible 2-layer SFTR DQI demo (v0.16+)

A self-contained kit that exercises the SFTR **Data Quality
Pack** on a 2-layer synthetic input set (TSR + TAR). Same
fixtures every time → same outputs every time → diff-friendly
for sanity-checking after upstream changes.

Sister kit to [`../emir-data-quality-pack/`](../emir-data-quality-pack/),
which exercises the 24 EMIR indicators on a 5-layer input set.

v0.16 ships **4 SFTR indicators** on the T2 layer of
`auth.079` + `auth.052`. The T3 margin layer, `auth.080`
reconciliation, and `auth.083` missing-collateral computers
are scheduled for v0.17 — providing those inputs to
`opendqi sftr data-quality-pack` today is parsed-and-discarded
(reserved input slots).

## The 2 inputs

| file | message | source | records |
|---|---|---|---|
| `tsr.xml` | `auth.079` SFTR Trade State Report | copy of [`../sftr/tr_state/auth079-sample.xml`](../sftr/tr_state/auth079-sample.xml) | 13 TSR records |
| `tar.xml` | `auth.052` SFTR Trade Activity Report | copy of [`../sftr/tr_activity/auth052-tar-sample.xml`](../sftr/tr_activity/auth052-tar-sample.xml) | 14 TAR records |

## Run the demo

```bash
bash examples/sftr-data-quality-pack/demo.sh
```

Writes 5 artefacts under `out/` :

| file | content |
|---|---|
| `report.html` | Coloured (green / amber / red) Data Quality Pack section + the existing Top Issues / by-severity / by-dimension blocks. |
| `summary.json` | Standard `ScanSummary` — files_processed, records_processed, quality_score, etc. |
| `issues.csv` | v1.0 stable 11-column granular issues from the SFTR check registry. |
| `indicators.csv` | v1.0 stable 11-column DQI table (4 rows). |
| `evidence.csv` | v1.0 stable 7-column drill-down evidence (≤ 20 rows per indicator). |

The script also `cat`s `out/indicators.csv` to stdout so you
see the indicator table without opening anything.

## The 4 SFTR indicators

| ID | Dimension | Layer |
|---|---|---|
| `DQI_COLLATERAL_VALUE_MISSING_SFTR` | completeness | TSR — outstanding rows with collateral identifier set but no collateral_value |
| `DQI_LOAN_VALUE_MISSING_SFTR` | completeness | TSR — outstanding rows with no loan_value |
| `DQI_LOAN_VALUE_STALE_SFTR` | timeliness | TSR — state_as_of older than `max_valuation_age_business_days` (TARGET2) |
| `DQI_TIM_REPORTING_LATE_SFTR` | timeliness | TAR — reporting_timestamp lag > `max_reporting_delay_hours` |

See [`../../docs/data-quality-pack.md#indicator-details--sftr`](../../docs/data-quality-pack.md#indicator-details--sftr)
for numerator/denominator detail.

## Reference snapshot

`expected/` contains the canonical `indicators.csv` +
`summary.json` (timestamps masked) produced by this kit on
v0.16.0. **Not** a golden test (the actual golden is at
`crates/opendqi-cli/tests/golden/sftr-data-quality-pack.*`)
— it's a human-readable reference that helps you spot
upstream changes.

```bash
bash demo.sh
diff out/indicators.csv expected/indicators.csv  # should be empty
```

## What this kit deliberately does NOT cover

- **CSV mapping** — the SFTR DQI pack is XML-paths-only on the
  CLI ; CSV inputs go through `opendqi.sftr.scan_table` (Python).
- **Custom thresholds** — see `docs/data-quality-pack.md`
  "Threshold configuration".
- **pyarrow.Table inputs from Python** — v0.16 SFTR Python
  binding is paths-only ; dual-input arrives in v0.17.
- **T3 margin / `auth.080` reconciliation / `auth.083`
  missing-collateral indicators** — scheduled for v0.17.

## Disclaimer

The DQI verdicts above are **internal data quality
indicators**, not regulatory verdicts. A `red` status is an
internal alert asking the firm to investigate — not a
declaration of non-compliance. See
[`../../docs/data-quality-pack.md#disclaimer--what-a-dqi-is-not`](../../docs/data-quality-pack.md#disclaimer--what-a-dqi-is-not)
for the full vocabulary discipline.
