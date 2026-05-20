# OpenDQI from Python

> **Status: preview (v0.12.x).**
> **Stable**: the v1.0 Arrow output contract for `result.issues`
> (11 cols, names + types frozen, byte-identical to the existing
> `issues.csv` produced by the CLI — locked in v0.12.0 P3 and
> tested by `crates/opendqi-py/tests/test_issues_schema.py::
> test_arrow_schema_matches_csv_golden`).
> **Evolving**: API additions in v0.13 (`scan_directory`,
> `tr_audit`, `collateral_audit`, `book_reconcile`, a Spark UDF
> namespace) — **additive**, no breaking changes planned.

## Install

```bash
pip install opendqi
```

Four wheels per release: Linux x86_64 + ARM64, macOS x86_64 + ARM64.
**abi3-py39** ⇒ a single wheel per target covers Python 3.9+
unchanged (forward-compatible to 3.14+). No Windows wheels —
consistent with the existing CI matrix.

For local development against an unreleased branch:

```bash
cd crates/opendqi-py
python3.12 -m venv .venv && source .venv/bin/activate
pip install --upgrade pip maturin pytest pyarrow
PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 maturin develop --release
```

## Three patterns in 3 minutes

The 3 entry points cover every realistic data-platform integration.
Each is symmetric across `opendqi.emir.*` and `opendqi.sftr.*`.

### Pattern 1 — Scan a normalized Parquet

When your pipeline already materialises canonical EMIR/SFTR
Parquet (e.g. via `opendqi {emir,sftr} normalize ... --out`):

```python
import opendqi

result = opendqi.emir.scan_parquet("/path/to/tsr.parquet")
print(result.summary)            # dict, same shape as summary.json
print(result.issues.num_rows)    # pyarrow.Table, v1.0 11-col schema
```

### Pattern 2 — Parse XML directly into Arrow, then scan

When you have ISO 20022 XML (`auth.030.001.{03,04}` EMIR or
`auth.052.001.02` SFTR) in hand and don't want a Parquet round-trip:

```python
import opendqi

table = opendqi.emir.parse_xml("/path/to/auth030.xml")
# `table` is a canonical-schema pyarrow.Table — identical to the
# Parquet output of `opendqi emir normalize`.

mapping = {name: name for name in table.column_names}  # identity
result = opendqi.emir.scan_table(table, mapping)
```

### Pattern 3 — Custom column mapping (the realistic warehouse path)

When your Arrow table comes from a custom source — your data
warehouse, a non-canonical CSV, a Polars frame from a feature
pipeline — the column names won't match the canonical EMIR field
names. The `mapping` dict reroutes each canonical field to your
actual column name:

```python
import opendqi, pyarrow as pa

table = pa.table({
    "TradeUTI":          ["U001", "U002"],
    "ValuationTimestamp": [...],
    "MaturityDate":       [...],
})

result = opendqi.emir.scan_table(table, mapping={
    "uti":                 "TradeUTI",
    "valuation_timestamp": "ValuationTimestamp",
    "maturity_date":       "MaturityDate",
    # unmapped canonical fields are emitted as None on every record;
    # EMIR.COMP.* checks surface the missingness naturally
})
```

The mapping direction (`canonical_field → user_column`) is the
same as the CSV mapping pattern in `opendqi emir scan --mapping
mapping.yml`. Mapping a canonical field to a column that doesn't
exist on the input table raises a loud, actionable error — no
silent all-None records.

**See [`examples/python/`](../examples/python/) for runnable
versions of all 3.**

## Public API surface

Six functions total (3 per regime), symmetric.

| Function | Returns | Inputs |
|---|---|---|
| `opendqi.emir.scan_parquet(path, *, normalize=False)` | `PyScanResult` | path to canonical EMIR Parquet |
| `opendqi.emir.scan_table(table, mapping, *, normalize=False)` | `PyScanResult` | pyarrow.Table / RecordBatch + mapping dict |
| `opendqi.emir.parse_xml(path)` | `pyarrow.Table` | path to `auth.030.001.{03,04}` |
| `opendqi.sftr.scan_parquet(...)`, `scan_table(...)`, `parse_xml(...)` | (symmetric) | SFTR equivalents |

Plus module-level:

| Attribute | Description |
|---|---|
| `opendqi.__version__` | semver string from Cargo.toml |
| `opendqi.emir`, `opendqi.sftr` | regime submodules |

## Output shapes

### `result.summary` — dict

Same 9 fields as the on-disk `summary.json` (locked by the M0.21
`IssueAggregator` contract):

```python
{
    "regime":              "emir" | "sftr",
    "files_processed":     int,
    "records_processed":   int,
    "issues_total":        int,
    "issues_by_severity":  {"info": int, "warning": int, "high": int, "critical": int},
    "issues_by_dimension": {"completeness": int, "validity": int,
                            "accuracy": int, "consistency": int,
                            "uniqueness": int, "timeliness": int},
    "quality_score":       float,  # 0..100, higher is better
    "started_at":          str,    # ISO 8601 RFC 3339
    "finished_at":         str,
}
```

### `result.issues` — `pyarrow.Table` (v1.0 stable schema)

**11 columns, all `Utf8`. Column names + order are the v1.0
stable contract — byte-identical to the CLI `issues.csv` output.**

| col | nullable | source |
|---|---|---|
| `check_id` | NO | `DqIssue::check_id` |
| `regime` | NO | `"emir"` / `"sftr"` |
| `severity` | NO | `"info"` / `"warning"` / `"high"` / `"critical"` |
| `dimension` | NO | 6 ESMA dimensions |
| `record_id` | YES | per-record stable id |
| `uti` | YES | when known |
| `field` | YES | implicated canonical field name |
| `value` | YES | implicated value as a string |
| `message` | NO | human-readable finding |
| `source_file` | YES | input file path |
| `evidence_json` | YES | `serde_json::to_string(&evidence)` |

### `result.normalized` — `pyarrow.Table` or `None`

Populated **only** when the call passed `normalize=True`. Schema
matches the canonical Parquet output of `opendqi {emir,sftr}
normalize` (Decimal128(38,10) / Date32 / Timestamp(μs,UTC) / Utf8)
— the public `opendqi_io::{emir_schema, sftr_schema}` defines it.

```python
result = opendqi.emir.scan_parquet("tsr.parquet", normalize=True)
result.normalized.num_rows   # records returned alongside the issues
```

## Integration patterns

### DuckDB

```python
import opendqi, duckdb

result = opendqi.emir.scan_parquet("tsr.parquet")
issues = result.issues   # pyarrow.Table

# DuckDB auto-detects Arrow tables in scope.
duckdb.sql("""
    SELECT severity, COUNT(*) AS n
    FROM issues
    GROUP BY 1
    ORDER BY n DESC
""").show()
```

### Polars

```python
import opendqi, polars as pl

result = opendqi.emir.scan_parquet("tsr.parquet")
df = pl.from_arrow(result.issues)
df.group_by("dimension").count().sort("count", descending=True)
```

### pandas

```python
import opendqi

result = opendqi.emir.scan_parquet("tsr.parquet")
df = result.issues.to_pandas()
df["check_id"].value_counts().head(10)
```

### Spark (via Arrow / Parquet handoff — *not* yet a native UDF)

Until v0.13 ships a dedicated `opendqi.spark` namespace with a
native `mapInPandas` UDF, the recommended pattern is to let Spark
write canonical Parquet and OpenDQI read it back:

```python
# Spark side: write canonical EMIR Parquet
spark_df.write.mode("overwrite").parquet("/tmp/opendqi/input")

# OpenDQI side: scan
result = opendqi.emir.scan_parquet("/tmp/opendqi/input")

# Spark side: read issues back as a DataFrame
issues_sdf = spark.createDataFrame(result.issues.to_pandas())
```

For batch pipelines, calling the CLI directly is also a fine
option:

```bash
opendqi emir scan /tmp/opendqi/input --out /tmp/opendqi/report
```

then on the Spark side:

```python
issues = spark.read.csv("/tmp/opendqi/report/issues.csv", header=True)
```

## Status & limitations

- **Preview / Beta.** Semver 0.x.y — MINOR bumps may extend the
  API additively. **Breaking changes to the v1.0 Arrow contract
  for `result.issues` are explicitly out of scope** (locked in
  v0.12.0 P3, parity-tested vs the CLI golden).
- **No Spark-native UDF yet.** Use the Arrow / Parquet handoff
  pattern above; the dedicated namespace is on the
  [v0.13 roadmap](python-roadmap.md).
- **No `scan_directory` yet.** Call `scan_parquet` per file in a
  Python `for` loop. Native in v0.13.
- **No cross-message subcommands from Python yet** (`tr_audit`,
  `collateral_audit`, `book_reconcile`). Use the CLI for those
  multi-file flows in v0.12.x. Python wrappers v0.13 (trivial
  follow-on — P4 already exposes every needed primitive).
- **No Python wrapping of the SQLite feedback-store workflow.**
  Use the CLI (`opendqi feedback list/resolve/stale/analytics`).
- **No PyPI publish of an in-process workflow store**; the
  bindings stay stateless. Lifecycle / feedback / book-vs-TR
  remain CLI flows in v0.12.x.
- **No Polars `LazyFrame` zero-copy fast path** — for now,
  `pl.from_arrow(result.issues)` materialises a regular `pl.
  DataFrame` (cheap, single-batch). Lazy-frame fast-path v0.13+.
- **No Windows wheels** — consistent with the existing
  Ubuntu+macOS CI matrix. Add a Windows runner to
  `.github/workflows/{ci,python-release}.yml` if you need it
  internally.

## Architecture

The bindings are a **thin wrapper** over the existing Rust core
— no parser, no check, no report builder is reimplemented. See
[`docs/python-roadmap.md`](python-roadmap.md) for:

- module layout (`crates/opendqi-py/`)
- reuse table (which Rust primitive each Python call composes)
- Arrow schema mapping field-by-field for `EmirRecord` (54 cols)
  and `SftrRecord` (31 cols)
- the v0.12 milestone breakdown (P0 → P5)
- the v0.13+ deferred items

## Where to go next

- [`examples/python/`](../examples/python/) — 3 runnable scripts +
  Jupyter notebook
- [`README.md`](../README.md) — project overview, CLI + UI surface,
  216 checks coverage
- [`CHANGELOG.md`](../CHANGELOG.md) — full release history
- [GitHub issues](https://github.com/PauFou/OpenDQI/issues) — bug
  reports + feature requests
