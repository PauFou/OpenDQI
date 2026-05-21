# OpenDQI from Python

> **Status: preview (v0.13.x).**
> **Stable**: the v1.0 Arrow output contract for `result.issues`
> (11 cols, names + types frozen, byte-identical to the existing
> `issues.csv` produced by the CLI — locked in v0.12.0 P3 and
> tested by `crates/opendqi-py/tests/test_issues_schema.py::
> test_arrow_schema_matches_csv_golden`).
> **v0.13.0 shipped (additive)**: 10 new entry points covering
> multi-file scans (`scan_directory`, `scan_files`) and the four
> cross-message workflows (`tr_audit`, `collateral_audit`,
> `book_reconcile`, `missing_collateral`), plus an experimental
> `opendqi.spark.scan_spark_dataframe` helper. Single-file
> functions from v0.12 (`scan_parquet`, `scan_table`, `parse_xml`)
> are unchanged.
> **Evolving (v0.14+)**: store-backed lifecycle wrapping, native
> Spark mapInPandas UDF, Polars LazyFrame fast path, custom
> CsvMapping date formats — **additive**, no breaking changes
> planned.

## Install

```bash
pip install opendqi
```

**Three wheels per release** : Linux x86_64 + ARM64, macOS ARM64
(Apple Silicon). **abi3-py39** ⇒ a single wheel per target covers
Python 3.9+ unchanged (forward-compatible to 3.14+).

**No macOS x86_64 (Intel) wheel** since v0.12.2 — the GitHub
`macos-13` runner is deprecated and free-tier jobs queue
indefinitely (we observed v0.12.0/v0.12.1 publishes hang 1h+
without starting). Intel-Mac users can either install via
`cargo install --git https://github.com/PauFou/OpenDQI --tag
v0.12.x opendqi-cli` for the CLI, or run the Linux wheel under
Rosetta 2 for the Python bindings. **No Windows wheels** —
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

## Multi-file scans (v0.13.0)

For pipelines that materialise more than one file per scan
(daily directory dump, mixed XML+Parquet inputs, etc.), v0.13
adds two aggregator entry points :

```python
# Walk a directory non-recursively, dispatch by extension,
# aggregate all records, scan as one batch.
result = opendqi.emir.scan_directory("/path/to/inputs/")

# Explicit list — same logic minus the discovery walk.
result = opendqi.emir.scan_files([
    "auth030.xml",
    "trades.parquet",
    "more_auth030.xml",
])

# Symmetric for SFTR.
result = opendqi.sftr.scan_directory("/path/to/sftr/inputs/")
```

Accepts `.xml` (firm-submission ISO 20022) and `.parquet` (the
canonical normalized schema). **`.csv` is rejected** since it
needs a column mapping; use `scan_table(pa.csv.read_csv(path),
mapping=…)` per file for that case.

## Cross-message workflows (v0.13.0)

The four cross-message CLI commands now have native Python
equivalents — same engine, same checks, single-call API :

```python
# 3-layer post-TR audit (TAR + TSR + feedback) + 3 EMIR.AUD.*
# cross-layer coherence checks. SFTR variant takes 2 layers.
result = opendqi.emir.tr_audit(
    tar="auth030.xml",
    tsr="auth107.xml",
    feedback="auth092.xml",
)
result = opendqi.sftr.tr_audit(tar="auth052.xml", tsr="auth079.xml")

# EMIR Article 11 collateral obligation — TSR ↔ MSR cross-ref
# by UTI; fires EMIR.COL.MISSING / EMIR.COL.STALE.
result = opendqi.emir.collateral_audit(
    tsr="auth107.xml",
    msr="auth109.xml",
)

# SFTR Missing Collateral Request — 2 base SFTR.MCR.* + 3
# optional cross-ref checks when --tsr is provided.
result = opendqi.sftr.missing_collateral(
    "auth083.xml",
    tsr="auth079.xml",   # optional — adds 3 cross-ref checks
)

# Internal book ↔ TR state reconciliation — 5 EMIR.BREC.* /
# SFTR.BREC.* mismatch checks. The `book` arg is dual-input :
# str path (.csv with mapping, or .parquet without) OR a
# pyarrow.Table/RecordBatch already in memory (always needs
# mapping).
result = opendqi.emir.book_reconcile(
    "book.csv",
    "auth107.xml",
    mapping={"uti": "trade_uti", "notional_amount": "notional", ...},
)
```

All cross-message workflows return the same `PyScanResult`
shape; check IDs are prefixed by layer (`EMIR.{COMP,VLD,...,
TST,FBK,AUD,COL,BREC,MCR}.…`) so per-layer filtering in Python
is a one-liner :

```python
aud_only = result.issues.filter(
    pa.compute.starts_with(result.issues.column("check_id"), "EMIR.AUD.")
)
```

**Store-backed lifecycle checks** (the CLI's `--store` flag)
are NOT yet wrapped in Python — use the CLI for cross-batch
lifecycle. v0.14+ may add it.

## Public API surface

| Function | Returns | Inputs |
|---|---|---|
| **Single-file** (v0.12.0) | | |
| `opendqi.emir.scan_parquet(path, *, normalize=False)` | `PyScanResult` | path to canonical EMIR Parquet |
| `opendqi.emir.scan_table(table, mapping, *, normalize=False)` | `PyScanResult` | pyarrow.Table / RecordBatch + mapping dict |
| `opendqi.emir.parse_xml(path)` | `pyarrow.Table` | path to `auth.030.001.{03,04}` |
| **Multi-file** (v0.13.0) | | |
| `opendqi.emir.scan_directory(path, *, normalize=False)` | `PyScanResult` | directory path; aggregates all .xml + .parquet |
| `opendqi.emir.scan_files(paths, *, normalize=False)` | `PyScanResult` | explicit list of paths |
| **Cross-message** (v0.13.0) | | |
| `opendqi.emir.tr_audit(*, tar, tsr, feedback)` | `PyScanResult` | 3 paths; 3 EMIR.AUD.* + per-layer |
| `opendqi.emir.collateral_audit(*, tsr, msr)` | `PyScanResult` | auth.107 + auth.109; EMIR.COL.* |
| `opendqi.emir.book_reconcile(book, tsr, *, mapping=None)` | `PyScanResult` | str path or pa.Table + auth.107; EMIR.BREC.* |
| `opendqi.sftr.{scan_parquet, scan_table, parse_xml, scan_directory, scan_files, tr_audit, book_reconcile, missing_collateral}(…)` | (symmetric, no `collateral_audit` — SFTR-side cross-collateral is `missing_collateral`) | |
| **Experimental** (v0.13.0) | | |
| `opendqi.spark.scan_spark_dataframe(df, *, regime, mapping, normalize=False)` | `pyspark.sql.DataFrame` | Spark DataFrame round-trip via pandas+Arrow; emits FutureWarning |

Plus module-level:

| Attribute | Description |
|---|---|
| `opendqi.__version__` | semver string from Cargo.toml |
| `opendqi.emir`, `opendqi.sftr`, `opendqi.spark` | submodules |

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

### Spark — `opendqi.spark` (v0.13.0 experimental)

v0.13.0 ships an **experimental** Python-only helper that does
the Spark→pandas→Arrow→scan_table→pandas→Spark round-trip in
one call :

```python
import opendqi

issues_sdf = opendqi.spark.scan_spark_dataframe(
    spark_df,
    regime="emir",
    mapping={"uti": "trade_uti", "valuation_timestamp": "MtmTs", ...},
)
# issues_sdf is a Spark DataFrame with the 11-column v1.0 issues
# schema. Emits a FutureWarning — signature may evolve in v0.14
# once a native mapInPandas UDF ships.
```

PySpark is **not** declared as a dependency of the `opendqi`
wheel (duck-typed import inside the helper) — install it
yourself: `pip install pyspark`.

For large batch pipelines, the Parquet handoff via CLI is still
the most robust pattern:

```python
spark_df.write.mode("overwrite").parquet("/tmp/opendqi/input")
# then in a shell:
#   opendqi emir scan /tmp/opendqi/input --out /tmp/opendqi/report
# back in PySpark:
issues = spark.read.csv("/tmp/opendqi/report/issues.csv", header=True)
```

## Status & limitations

- **Preview / Beta.** Semver 0.x.y — MINOR bumps may extend the
  API additively. **Breaking changes to the v1.0 Arrow contract
  for `result.issues` are explicitly out of scope** (locked in
  v0.12.0 P3, parity-tested vs the CLI golden).
- **`opendqi.spark` is EXPERIMENTAL** in v0.13. The Spark→
  pandas→Arrow round-trip works but emits a `FutureWarning`.
  The signature may evolve in v0.14 once a native
  `mapInPandas` UDF ships (partition-friendly, zero-copy).
- **No Python wrapping of the SQLite feedback-store / lifecycle
  workflow.** `--store` and the lifecycle / feedback
  list/resolve/stale/analytics commands stay CLI-only. The
  Python `tr_audit` runs the cross-layer audit but NOT cross-
  batch lifecycle checks (which need store-loaded prior records).
- **`book_reconcile` accepts CSV / Parquet / pyarrow.Table** for
  the book side, but `CsvMapping` date/datetime format strings
  use the defaults (`%Y-%m-%d` and RFC 3339). Non-default formats
  go through the CLI with `--mapping` YAML. Will be exposed in
  Python in v0.14+ if asked.
- **No `mar_scan` / `msr_scan` / `recon_stats` / `warnings`
  single-message Python wrappers.** These are single-message TR
  scans that don't add much over `scan_table(pa.from_…(message))`.
  Add in v0.14+ if a user pattern emerges.
- **No Polars `LazyFrame` zero-copy fast path** — for now,
  `pl.from_arrow(result.issues)` materialises a regular `pl.
  DataFrame` (cheap, single-batch). Lazy-frame fast-path v0.14+.
- **No Windows wheels** — consistent with the existing
  Ubuntu+macOS CI matrix.

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
