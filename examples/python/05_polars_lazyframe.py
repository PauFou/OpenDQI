#!/usr/bin/env python3
"""
OpenDQI Python quickstart — Pattern 5 (v0.14.0): Polars LazyFrame
fast path with column push-down.

For wide LazyFrames where only a handful of columns are mapped
to canonical EMIR/SFTR fields, `opendqi.polars.scan_lazyframe`
selects only the mapped columns BEFORE materializing — a real
speedup vs collecting the full frame.

Requires: pip install opendqi[polars]
Run from the repo root:

    python examples/python/05_polars_lazyframe.py
"""
from __future__ import annotations

import json
from collections import Counter
from pathlib import Path

import opendqi


REPO_ROOT = Path(__file__).resolve().parents[2]
EMIR_TAR_XML = REPO_ROOT / "examples" / "quickstart-emir" / "auth030-tar.xml"


def main() -> None:
    try:
        import polars as pl
    except ImportError:
        raise SystemExit(
            "✗ polars not installed. Run: pip install opendqi[polars]"
        )

    if not EMIR_TAR_XML.exists():
        raise SystemExit(f"✗ fixture missing: {EMIR_TAR_XML}")

    # Step 1 — get a canonical EMIR Arrow Table.
    table = opendqi.emir.parse_xml(str(EMIR_TAR_XML))
    print(f"parsed {table.num_rows} records, {len(table.column_names)} columns")

    # Step 2 — wrap into a Polars LazyFrame. Simulate a "wide"
    # frame by adding a few junk columns the mapping won't touch.
    df = pl.from_arrow(table)
    for i in range(3):
        df = df.with_columns(pl.lit(f"junk_value_{i}").alias(f"__junk_col_{i}"))
    lf = df.lazy()
    print(f"wide LazyFrame: {len(df.columns)} columns "
          f"({3} of them are unused junk)")

    # Step 3 — only the columns referenced in `mapping` get
    # materialized (push-down select). Junk columns are dropped
    # before .collect() ever runs.
    mapping = {name: name for name in table.column_names}
    result = opendqi.polars.scan_lazyframe(
        lf, regime="emir", mapping=mapping
    )

    print("\n=== summary ===")
    print(json.dumps(result.summary, indent=2, default=str))

    print("\n=== top 5 check IDs by frequency ===")
    counts = Counter(result.issues.column("check_id").to_pylist())
    for check_id, n in counts.most_common(5):
        print(f"  {n:>3}× {check_id}")


if __name__ == "__main__":
    main()
