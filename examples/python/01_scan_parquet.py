#!/usr/bin/env python3
"""
OpenDQI Python quickstart — Pattern 1: scan a normalized Parquet.

The simplest possible entry point: hand `opendqi.emir.scan_parquet`
a path to a canonical EMIR Parquet (produced by
`opendqi emir normalize ... --out`), get back a `PyScanResult` with
a `summary` dict and an `issues` pyarrow.Table.

Run:
    pip install opendqi
    python examples/python/01_scan_parquet.py

If the canonical fixture Parquet does not exist yet, this script
generates it on the fly via the `opendqi` CLI binary (assumed to
be in PATH or at `target/debug/opendqi`).
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

import opendqi


REPO_ROOT = Path(__file__).resolve().parents[2]
CSV_FIXTURE = REPO_ROOT / "examples" / "emir" / "sample.csv"
CSV_MAPPING = REPO_ROOT / "examples" / "emir" / "sample_mapping.yml"
PARQUET_OUT = REPO_ROOT / "target" / "examples" / "sample.parquet"


def find_opendqi_bin() -> str | None:
    """Locate the `opendqi` CLI: PATH first, then debug build."""
    on_path = shutil.which("opendqi")
    if on_path:
        return on_path
    debug = REPO_ROOT / "target" / "debug" / "opendqi"
    if debug.exists():
        return str(debug)
    return None


def ensure_parquet() -> Path:
    """Generate the canonical EMIR Parquet on first run."""
    if PARQUET_OUT.exists():
        return PARQUET_OUT
    bin_path = find_opendqi_bin()
    if bin_path is None:
        sys.exit(
            "✗ opendqi CLI not found.\n"
            "  → install it (cargo install --git ...) or `cargo build` in the repo, "
            "or write your own Parquet via `opendqi emir normalize`."
        )
    PARQUET_OUT.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        [bin_path, "emir", "normalize",
         str(CSV_FIXTURE), "--mapping", str(CSV_MAPPING),
         "--out", str(PARQUET_OUT)],
        check=True,
        env={**os.environ, "RUST_LOG": "warn"},
    )
    return PARQUET_OUT


def main() -> None:
    parquet = ensure_parquet()

    result = opendqi.emir.scan_parquet(str(parquet))

    print("=== summary ===")
    print(json.dumps(result.summary, indent=2, default=str))

    print("\n=== issues (pyarrow.Table) ===")
    print(f"rows={result.issues.num_rows}  cols={result.issues.num_columns}")
    print(f"schema: {result.issues.column_names}")

    print("\n=== first 5 issues (as pandas) ===")
    # `result.issues` is a pyarrow.Table — `.to_pandas()` is the
    # standard escape hatch for pandas users. Polars users would
    # call `pl.from_arrow(result.issues)`. DuckDB users can
    # `duckdb.sql("SELECT * FROM issues").df()` thanks to
    # automatic Arrow integration.
    try:
        df = result.issues.to_pandas()
        print(df[["check_id", "severity", "dimension", "uti"]].head(5).to_string(index=False))
    except ImportError:
        print("(install pandas for .to_pandas() output — pyarrow alone is enough for scanning)")


if __name__ == "__main__":
    main()
