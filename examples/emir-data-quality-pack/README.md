# `examples/emir-data-quality-pack/` — reproducible 5-layer DQI demo (v0.16+)

A self-contained kit that exercises the EMIR **Data Quality
Pack** on a 5-layer synthetic input set. Same fixtures every
time → same outputs every time → diff-friendly for
sanity-checking after upstream changes.

v0.16 ships **24 EMIR indicators** (10 v0.15 + 14 v0.16).
7 of the v0.16 indicators are auth.091 cross-CP rollups
whose CLI wiring (`--recon-stats` / `--reconciliation`) is
not yet threaded through `data-quality-pack` — they
self-report `not_applicable` here. The 17 single-layer DQIs
all compute normally.

Sister kit to [`../quickstart-emir/`](../quickstart-emir/),
which ships only 3 layers (TSR + TAR + Feedback) and runs
the row-level checks. This kit adds MSR + MAR so the
**cross-table** DQIs (`DQI_COL_*`) compute meaningfully.

## The 5 inputs

| file | message | source | UTIs |
|---|---|---|---|
| `tsr.xml` | `auth.107` Trade State Report | copy of [`../quickstart-emir/auth107-tsr.xml`](../quickstart-emir/auth107-tsr.xml) | 7 outstanding, prefix `OPENDQI-TSR-*`. Only `OPENDQI-TSR-CLEAN-0001` carries a `CollPrtflCd` → it is the *only* row in the `DQI_COL_MISSING_STATE` denominator. |
| `tar.xml` | `auth.030` Trade Activity Report | copy of [`../quickstart-emir/auth030-tar.xml`](../quickstart-emir/auth030-tar.xml) | 5 records |
| `feedback.xml` | `auth.092` Validation Feedback | copy of [`../quickstart-emir/auth092-feedback.xml`](../quickstart-emir/auth092-feedback.xml) | 2 records, both `feedback_type=Rejected` |
| **`msr.xml`** | `auth.109` Margin State Report | **NEW** synthetic (this kit) | 4 records, UTIs aligned with the TSR — see "UTI alignment" below |
| **`mar.xml`** | `auth.108` Margin Activity Report | **NEW** synthetic (this kit) | 3 records, UTIs aligned with the TSR |

## UTI alignment (what makes the demo pedagogical)

| TSR UTI | in MSR? | in MAR? | What this exercises |
|---|---|---|---|
| `OPENDQI-TSR-CLEAN-0001` | ❌ omitted | ✓ correction event | **`DQI_COL_MISSING_STATE` numerator++** — collateralised in TSR (the only such row), absent from MSR → 1/1 = **red** |
| `OPENDQI-TSR-STALE-0002` | ✓ with `state_as_of=2026-05-15` | ✓ | **`DQI_COL_STALE_STATE` numerator++** — 6 days old vs `--as-of 2026-05-21`, threshold `collateral_max_age_days = 1` |
| `OPENDQI-TSR-NOVAL-0003` | ✓ with all 4 margin amounts zero | — | **`DQI_COL_ALL_ZERO` numerator++** |
| `OPENDQI-TSR-DUP-0006` | ✓ normal | ✓ | Contributes to denominators ; no DQI fires |
| `OPENDQI-MSR-EXTRA-0008` | ✓ MSR-only | — | Demonstrates that the TSR↔MSR join is **by UTI** — extras on the MSR side don't change `DQI_COL_MISSING_STATE` (its denominator is TSR-driven) |

## Run the demo

```bash
bash examples/emir-data-quality-pack/demo.sh
```

Writes 5 artefacts under `out/` :

| file | content |
|---|---|
| `report.html` | Coloured (green / amber / red) Data Quality Pack section + the existing Top Issues / by-severity / by-dimension blocks. |
| `summary.json` | Standard `ScanSummary` — files_processed, records_processed, quality_score, etc. |
| `issues.csv` | v1.0 stable 11-column granular issues from the 216 checks. |
| `indicators.csv` | v1.0 stable 11-column DQI table (24 rows). |
| `evidence.csv` | v1.0 stable 7-column drill-down evidence (≤ 20 rows per indicator). |

The script also `cat`s `out/indicators.csv` to stdout so you
see the indicator table without opening anything.

## Reference snapshot

`expected/` contains the canonical `indicators.csv` +
`summary.json` (timestamps masked) produced by this kit on
v0.16.0. **Not** a golden test (not wired into the
`opendqi-cli` harness) — it's a human-readable reference
that helps you spot upstream changes.

```bash
bash demo.sh
diff out/indicators.csv expected/indicators.csv  # should be empty
```

If the diff is non-empty after an upstream change, either :
- a DQI formula / threshold default intentionally moved →
  re-snapshot `expected/` and commit the new reference, or
- the change unintentionally drifted → investigate.

`issues.csv` / `evidence.csv` / `report.html` are not
snapshotted in `expected/` (`issues.csv` is ~200 rows and
volatile to any check change ; `report.html` is minijinja-
rendered with timestamps).

## What this kit deliberately does NOT cover

- **CSV mapping** — see `../emir/sample.csv` + `../emir/sample_mapping.yml` for the CSV path.
- **Custom thresholds** — see `docs/data-quality-pack.md` "Threshold configuration".
- **Spark / Polars / pyarrow.Table inputs** — see `examples/python/07_data_quality_pack.py` + `docs/data-quality-pack.md` "Python API" / "Spark API".
- **SFTR** — see the sister kit [`../sftr-data-quality-pack/`](../sftr-data-quality-pack/) which exercises the 4 T2-layer SFTR indicators v0.16 ships.

## Disclaimer

The DQI verdicts above are **internal data quality indicators**, not regulatory verdicts. A `red` status is an internal alert asking the firm to investigate — not a declaration of non-compliance. See [`../../docs/data-quality-pack.md#disclaimer--what-a-dqi-is-not`](../../docs/data-quality-pack.md#disclaimer--what-a-dqi-is-not) for the full vocabulary discipline.
