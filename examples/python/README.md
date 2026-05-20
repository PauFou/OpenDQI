# OpenDQI — Python quickstart kit

Three copy-pasteable scripts showcasing the three primary patterns
of the [`opendqi`](https://pypi.org/p/opendqi) Python bindings. Each
script is autonomous (~30 lines), runs against the synthetic
fixtures shipped in this repo, and prints a `summary` dict + a
slice of the `issues` Arrow table.

## Install

```bash
pip install opendqi
```

Available from v0.12.1. Wheels for Linux x86_64 + ARM64 and macOS
x86_64 + ARM64 (abi3-py39 → one wheel covers Python 3.9+
unchanged). No Windows wheels.

For local development against an unreleased branch:

```bash
cd crates/opendqi-py
python3.12 -m venv .venv && source .venv/bin/activate
pip install --upgrade pip maturin pytest pyarrow
PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 maturin develop --release
```

## The three patterns

```bash
# 1. Scan a normalized Parquet file
python examples/python/01_scan_parquet.py

# 2. Parse an ISO 20022 XML in memory, then scan with an
#    identity mapping (no Parquet roundtrip)
python examples/python/02_parse_xml_then_scan.py

# 3. Scan a user-named Arrow table via a custom column mapping
#    (the realistic data-warehouse path)
python examples/python/03_custom_mapping.py
```

| Script | What it shows | Best for |
|---|---|---|
| **`01_scan_parquet.py`** | Single-call entry point on an existing canonical Parquet | Pipelines that already materialise normalized data |
| **`02_parse_xml_then_scan.py`** | `parse_xml → scan_table` chain, no disk intermediate | One-off interactive audits + in-memory data platforms |
| **`03_custom_mapping.py`** | `mapping={canonical: user_col}` for renamed columns | Real-world data warehouses where columns aren't named per ESMA convention |

## What you get back

```python
result.summary       # dict — same shape as summary.json (9 fields)
result.issues        # pyarrow.Table — v1.0 stable 11-column schema
result.normalized    # pyarrow.Table | None (when normalize=True)
```

The 11-column `issues` schema is the **v1.0 stable contract** —
locked in v0.12.0 P3 (commit `54bbf52`), tested against the CLI
golden by `test_arrow_schema_matches_csv_golden`. **Any change is
a breaking change**.

## Going further

- [`docs/python.md`](../../docs/python.md) — the full quickstart
  with integration patterns for DuckDB / Polars / pandas / Spark.
- [`docs/python-roadmap.md`](../../docs/python-roadmap.md) — the
  architecture spec (module layout, Arrow schema mapping, v0.13+
  roadmap).
- [`examples/python/quickstart.ipynb`](quickstart.ipynb) — same 3
  patterns as an executable Jupyter notebook.
