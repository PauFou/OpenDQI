# `examples/sftr-data-quality-pack/` — reproducible 5-layer SFTR DQI demo (v0.17+)

A self-contained kit that exercises the SFTR **Data Quality
Pack** on a 5-layer synthetic input set. Same fixtures every
time → same outputs every time → diff-friendly for sanity-
checking after upstream changes.

Sister kit to [`../emir-data-quality-pack/`](../emir-data-quality-pack/),
which exercises the 24 EMIR indicators on a 5-layer input set.

v0.17 ships **16 SFTR indicators** across the 5 input layers
(TSR + TAR + reconciliation + missing-collateral + MSR) — the
full mirror of the EMIR-side DQI pack adapted to the real
ESMA SFTR message set (`auth.052` / `079` / `080` / `083` /
`085`).

## The 5 inputs

| file | message | source | records |
|---|---|---|---|
| `tsr.xml` | `auth.079` SFTR Trade State Report | copy of [`../sftr/tr_state/auth079-sample.xml`](../sftr/tr_state/auth079-sample.xml) | 13 TSR records |
| `tar.xml` | `auth.052` SFTR Trade Activity Report | copy of [`../sftr/tr_activity/auth052-tar-sample.xml`](../sftr/tr_activity/auth052-tar-sample.xml) | 14 TAR records |
| `reconciliation.xml` | `auth.080` SFTR Reconciliation Status Advice | copy of [`../sftr/reconciliation/auth080-sample.xml`](../sftr/reconciliation/auth080-sample.xml) | 3 reconciliation records |
| `missing_collateral.xml` | `auth.083` SFTR Missing Collateral Request | copy of [`../sftr/missing_collateral/auth083-sample.xml`](../sftr/missing_collateral/auth083-sample.xml) | 2 MCR records |
| `msr.xml` | `auth.085` SFTR Margin Data Transaction State Report | copy of [`../sftr/margin_state/auth085-sample.xml`](../sftr/margin_state/auth085-sample.xml) | 5 MSR records (CCP-cleared portfolios) |

## Run the demo

```bash
bash examples/sftr-data-quality-pack/demo.sh
```

Writes 5 artefacts under `out/` :

| file | content |
|---|---|
| `report.html` | Coloured (green / amber / red) Data Quality Pack section + the existing Top Issues / by-severity / by-dimension blocks. |
| `summary.json` | Standard `ScanSummary` — files_processed, records_processed, quality_score, etc. |
| `issues.csv` | v1.0 stable 11-column granular issues from the SFTR check registry (including the v0.17 SFTR.T3.* checks when `--msr` fires). |
| `indicators.csv` | v1.0 stable 11-column DQI table (16 rows). |
| `evidence.csv` | v1.0 stable 7-column drill-down evidence (≤ 20 rows per indicator). |

The script also `cat`s `out/indicators.csv` to stdout so you
see the indicator table without opening anything.

## The 16 SFTR indicators by layer

| Layer | Count | Indicators |
|---|---|---|
| TSR (`auth.079`) | **6** | `DQI_COLLATERAL_VALUE_MISSING_SFTR`, `DQI_LOAN_VALUE_MISSING_SFTR`, `DQI_LOAN_VALUE_STALE_SFTR`, `DQI_HAIRCUT_ANOMALY_SFTR` (v0.17), `DQI_LEI_MISSING_SFTR` (v0.17), `DQI_UNDER_COLLATERALIZATION_SFTR` (v0.17) |
| TAR (`auth.052`) | **1** | `DQI_TIM_REPORTING_LATE_SFTR` |
| Reconciliation (`auth.080`) | **4** | `DQI_PAIRING_RATE_SFTR`, `DQI_RECONCILIATION_RATE_SFTR`, `DQI_UNPAIRED_TRADES_RATE_SFTR`, `DQI_FIELD_MISMATCH_RATE_SFTR` (all v0.17) |
| Missing-collateral (`auth.083`) | **1** | `DQI_MCR_OPEN_REQUESTS_SFTR` (v0.17) |
| MSR (`auth.085`) | **4** | `DQI_T3_MARGIN_POSTED_MISSING_SFTR`, `DQI_T3_MARGIN_RECEIVED_MISSING_SFTR`, `DQI_T3_EXCESS_COLLATERAL_USE_SFTR`, `DQI_T3_MARGIN_STALE_SFTR` (all v0.17) |

The MSR layer also triggers 6 granular `SFTR.T3.*` per-record
checks (`IM/VM_POSTED/RECEIVED_MISSING` for partial-side
reporting, `MARGIN_NEGATIVE`, `MARGIN_CURRENCY_MISSING`)
co-produced into `issues.csv` alongside the aggregated DQIs.

See [`../../docs/data-quality-pack.md#indicator-details--sftr`](../../docs/data-quality-pack.md#indicator-details--sftr)
for full numerator / denominator / threshold detail per DQI,
and [`../../docs/auth-messages/sftr-auth085.md`](../../docs/auth-messages/sftr-auth085.md)
for the auth.085 XSD path mapping.

## Reference snapshot

`expected/` contains the canonical `indicators.csv` +
`summary.json` (timestamps masked) produced by this kit on
v0.17.0. **Not** a golden test (the actual golden is at
`crates/opendqi-cli/tests/golden/sftr-data-quality-pack-full.*`)
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
- **pyarrow.Table inputs from Python** — v0.17 SFTR Python
  binding is still paths-only ; dual-input on the SFTR side
  is scheduled for v0.18 (EMIR side already supports it).
- **CLI/Python wiring of the 7 auth.091 EMIR cross-CP DQIs** —
  v0.16 honest scope carry-over (computers ship in core but
  the `--recon-stats` flag is not yet on `data-quality-pack`).
- **DQI history / trend store** — v0.18+.

## Disclaimer

The DQI verdicts above are **internal data quality
indicators**, not regulatory verdicts. A `red` status is an
internal alert asking the firm to investigate — not a
declaration of non-compliance. See
[`../../docs/data-quality-pack.md#disclaimer--what-a-dqi-is-not`](../../docs/data-quality-pack.md#disclaimer--what-a-dqi-is-not)
for the full vocabulary discipline.
