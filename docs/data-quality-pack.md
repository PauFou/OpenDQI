# Data Quality Pack (v0.15+)

The Data Quality Pack is OpenDQI's **committee-readable**
view on top of the 216 granular checks. Where the per-row
`DqIssue` stream is forensic (159 unique check IDs, 6
dimensions), the DQI pack rolls them up into **10
regulator-style indicators** with `numerator / denominator /
rate / threshold / status`, plus drill-down evidence.

Same scan, two views — the granular issue stream is
**co-produced**, never replaced. `issues.csv` keeps carrying
the per-row defects ; `indicators.csv` + `evidence.csv` are
new outputs.

## 30-second example

```bash
opendqi emir data-quality-pack \
  --tsr  examples/quickstart-emir/auth107-tsr.xml \
  --tar  examples/quickstart-emir/auth030-tar.xml \
  --feedback examples/quickstart-emir/auth092-feedback.xml \
  --as-of 2026-05-21 \
  --out  ./pack/
```

Output under `./pack/` :

| file | content |
|---|---|
| `report.html` | Coloured (green/amber/red) indicator table + the existing Top Issues / by-severity / by-dimension sections. |
| `summary.json` | Standard `ScanSummary` — unchanged shape since v0.10. |
| `issues.csv` | v1.0 stable 11-column granular issues (unchanged contract since v0.12). |
| `indicators.csv` | **NEW** — v1.0 stable 11 columns, one row per shipped DQI. |
| `evidence.csv` | **NEW** — v1.0 stable 7 columns, ≤ 20 evidence rows per DQI. |

Stdout summary :

```
Data Quality Pack: 4/10 indicators computed (3 red, 0 amber).
Granular: 215 issues, score 46.7/100.
Report: ./pack/report.html
```

## The 10 indicators

| ID | Layer(s) | Dimension | Numerator | Denominator | Default thresholds (amber / red) |
|---|---|---|---|---|---|
| `DQI_VAL_MISSING` | TSR | completeness | outstanding TSR rows with no / zero valuation | outstanding TSR rows | 0.5 % / 2 % |
| `DQI_VAL_STALE` | TSR | timeliness | rows with valuation older than `max_valuation_age_business_days` vs `as_of` | rows with `valuation_timestamp` set | 1 % / 5 % |
| `DQI_COL_MISSING_STATE` | TSR + MSR | completeness | outstanding-collateralised TSR rows with no MSR row | outstanding-collateralised TSR rows | 1 % / 5 % |
| `DQI_COL_ALL_ZERO` | MSR | accuracy | MSR rows where all 4 margin fields are zero / NULL | MSR rows | 2 % / 10 % |
| `DQI_COL_STALE_STATE` | MSR | timeliness | rows with `state_as_of` older than `collateral_max_age_days` | rows with `state_as_of` set | 5 % / 20 % |
| `DQI_REJ_RATE` | Feedback | accuracy | rejected feedback records | total feedback records (proxy) | 1 % / 5 % |
| `DQI_REJ_REPEAT_UTI` | Feedback | accuracy | distinct UTIs rejected ≥ 2× | distinct rejected UTIs | 0.5 % / 2 % |
| `DQI_TIM_REPORTING_LATE` | TAR | timeliness | gap > `max_reporting_delay_hours` | rows with both timestamps | 5 % / 20 % |
| `DQI_CONF_MISSING` | TAR (gated) | completeness | rows with no `confirmation_timestamp` | TAR rows | 5 % / 20 % |
| `DQI_REC_STATUS_UNPAIRED` | TAR/TSR (gated) | consistency | rows tagged unpaired / unreconciled | rows with status set | 5 % / 20 % |

**Status mapping** (common to all 10) :
- `rate ≤ amber_threshold` → **green**
- `amber_threshold < rate ≤ red_threshold` → **amber**
- `rate > red_threshold` → **red**
- `denominator == 0` OR input layer not provided OR gated
  field unmapped → **not_applicable** (`rate = null`, no
  evidence)

## Indicator details

### `DQI_VAL_MISSING`

**Question**: *"What share of my outstanding trades are reported with no valuation?"*

- **Numerator**: TSR records where `valuation_amount` is
  `None` or `0`, AND `status` is not `MATURED` /
  `TERMINATED`, AND `termination_date` is unset.
- **Denominator**: same predicate but without the
  no-valuation filter (i.e. outstanding TSR rows).
- **Evidence**: top-20 offenders sorted by
  `(source_file, uti)` ascending — UTI + reporting
  counterparty + source file. `observed_value` is the
  serialised `valuation_amount` (empty when `None`).

### `DQI_VAL_STALE`

**Question**: *"How fresh are my valuations relative to today?"*

- **Numerator**: TSR records where
  `valuation_timestamp < as_of - max_valuation_age_business_days`.
- **Denominator**: TSR records with `valuation_timestamp`
  set (rows without are excluded — already counted by
  `DQI_VAL_MISSING`).
- **Note (honest scope limit)** : v0.15 uses calendar days
  as a proxy for business days. A business-day-calendar
  refinement is scheduled for v0.16. The
  `max_valuation_age_business_days = 1` default thus
  matches "yesterday or earlier" — close enough on the
  measured workloads.
- **Evidence**: oldest timestamps first.

### `DQI_COL_MISSING_STATE`

**Question**: *"Article 11 — for my collateralised outstanding trades, is the margining state actually reported?"*

- Mirrors the per-row `EMIR.COL.MISSING` check at the
  indicator level — same `compute_collateral_emir_issues`
  UTI-indexing pattern in `dq/collateral_audit.rs`.
- **Denominator**: outstanding TSR records with
  `collateral_portfolio_code` set.
- **Numerator**: same, minus those whose UTI is present in
  the MSR.
- **Evidence**: missing UTIs first, with portfolio code as
  `observed_value`.

### `DQI_COL_ALL_ZERO`

**Question**: *"Are some MSR rows actually empty (signalling a UCOL trade misreported or a margining gap)?"*

- **Numerator**: MSR rows where all 4 margin fields
  (`initial_margin_posted_current`,
  `initial_margin_collected_current`,
  `variation_margin_posted_current`,
  `variation_margin_collected_current`) are `None` or `0`.
- **Denominator**: all MSR rows.

### `DQI_COL_STALE_STATE`

**Question**: *"Are the margining snapshots up-to-date?"*

- Mirrors `EMIR.COL.STALE` at the indicator level. Same
  `collateral_max_age_days` threshold (default 1 calendar
  day).
- **Numerator**: MSR rows where
  `state_as_of < as_of - collateral_max_age_days`.
- **Denominator**: MSR rows with `state_as_of` set.

### `DQI_REJ_RATE`

**Question**: *"What share of what the TR sends back are rejections?"*

- **Numerator**: `auth.092` records with
  `feedback_type == Rejected`.
- **Denominator (honest scope)**: total feedback records.
  The *true* denominator would be "number of submissions",
  which `auth.092` alone doesn't carry. v0.15 ships the
  proxy ; a real submission-count denominator is gated on
  the v0.16 store-backed workflow.

### `DQI_REJ_REPEAT_UTI`

**Question**: *"Are some UTIs chronically rejected (signalling a non-fixable structural issue)?"*

- **Numerator**: distinct UTIs that appear ≥ 2 times in
  `auth.092` with `feedback_type == Rejected`.
- **Denominator**: distinct UTIs with at least one
  rejection.
- **Evidence**: worst (most-rejected) first ; `observed_value` carries the count.

### `DQI_TIM_REPORTING_LATE`

**Question**: *"Are TAR submissions arriving within T+1?"*

- Mirrors `EMIR.TIM.LATE_REPORTING` at the indicator level.
- **Numerator**: TAR records where
  `(reporting_timestamp - execution_timestamp) > max_reporting_delay_hours`.
- **Denominator**: TAR records with both timestamps set.
- **Evidence**: biggest delay first ; `observed_value`
  carries the delay in hours (e.g. `"30h"`).

### `DQI_CONF_MISSING` (gated)

**Question**: *"Are confirmations being captured?"*

- Reads `raw_fields["confirmation_timestamp"]` on each TAR
  record.
- **Gated**: returns `status: not_applicable` if the field
  is not present in the mapping (CLI / Python detection)
  or never observed non-empty.

### `DQI_REC_STATUS_UNPAIRED` (gated)

**Question**: *"Of records carrying a TR-provided reconciliation status, how many are unpaired?"*

- Reads `raw_fields["reconciliation_status"]`. Token set
  matched (case-insensitive): `unpaired`, `not_paired`,
  `unrec`, `unreconciled`, `unmatched`.
- **Gated** like `DQI_CONF_MISSING`.

## Threshold configuration

Override per-indicator via YAML config :

```yaml
# my-thresholds.yml
dqi:
  DQI_VAL_MISSING:
    amber: 0.001   # 0.1 %
    red:   0.005   # 0.5 %
  DQI_VAL_STALE:
    amber: 0.005
    red:   0.02
  # other indicators inherit the shipped defaults
```

Use it :

```bash
opendqi emir data-quality-pack ... --config my-thresholds.yml
```

Missing indicator entries fall back to the shipped
defaults from `opendqi_core::default_dqi_thresholds()`,
then to the loose
`DqiThresholdPair::default { amber: 0.05, red: 0.20 }`.

## Python API

```python
import opendqi

result = opendqi.emir.data_quality_pack(
    tsr="auth107-tsr.xml",       # or pyarrow.Table
    tar="auth030-tar.xml",       # or pyarrow.Table
    msr="auth109-msr.xml",       # or pyarrow.Table
    feedback="auth092.xml",      # or pyarrow.Table
    mar="auth108-mar.xml",       # paths-only in v0.15
    mappings={                   # required when an input is a pyarrow.Table
        "tsr": {"uti": "TradeUti", "status": "Status", ...},
    },
    as_of="2026-05-21",          # defaults to today (UTC)
)

# v1.0 stable Arrow tables
result.indicators   # pyarrow.Table, 10 rows × 11 cols
result.evidence     # pyarrow.Table, ≤ 200 rows × 7 cols
result.issues       # pyarrow.Table, granular (same contract as v0.12+)

# dict shaped like summary.json
result.summary

# Write the 5 artefacts the CLI writes
result.report("./pack/")
```

## Spark API (EXPERIMENTAL — v0.15.0)

```python
import opendqi.spark.emir

result = opendqi.spark.emir.data_quality_pack(
    tsr=spark_df_tsr,            # collected at the driver
    feedback="auth092.xml",      # str paths still OK (mix-and-match)
    mappings={"tsr": {"uti": "TradeId", "status": "Status"}},
    as_of="2026-05-21",
)
# Same PyDqiPackResult as the core function.
```

**Honest** :
- Collect-then-call ; does not scale beyond driver-RAM.
- Native partition-aware joins = v0.16.
- `FutureWarning` emitted on every call.
- PySpark is optional : `pip install opendqi[spark]`.

## v1.0 Arrow schemas (locked)

### `indicators` schema (11 cols)

| column | type | nullable |
|---|---|---|
| `indicator_id` | Utf8 | false |
| `regime` | Utf8 | false |
| `dimension` | Utf8 | false |
| `table_scope` | Utf8 | false |
| `numerator` | UInt64 | false |
| `denominator` | UInt64 | false |
| `rate` | Float64 | true |
| `threshold_amber` | Float64 | true |
| `threshold_red` | Float64 | true |
| `status` | Utf8 | false |
| `description` | Utf8 | false |

### `evidence` schema (7 cols)

| column | type | nullable |
|---|---|---|
| `indicator_id` | Utf8 | false |
| `uti` | Utf8 | false |
| `counterparty` | Utf8 | true |
| `asset_class` | Utf8 | true |
| `source_file` | Utf8 | true |
| `observed_value` | Utf8 | true |
| `explanation` | Utf8 | false |

Both contracts are pinned by parity tests in
`crates/opendqi-py/tests/test_data_quality_pack.py` against
the on-disk CLI goldens. Any breaking change requires a
major version bump of the bindings.

## What v0.15 deliberately does NOT do

- Mirror SFTR (v0.16).
- Native partition-aware Spark (v0.16).
- DQI history / trend tracking via the SQLite store (v0.16).
- A submission-count-aware `DQI_REJ_RATE` denominator (v0.16+).
- Business-day calendar awareness for `DQI_VAL_STALE` (v0.16).
- New ISO 20022 messages, new compression / dispute /
  IM-cadence indicators (out of scope — see
  `docs/positioning.md`).
