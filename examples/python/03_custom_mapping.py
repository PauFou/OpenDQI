#!/usr/bin/env python3
"""
OpenDQI Python quickstart — Pattern 3: custom column mapping.

When your Arrow table comes from a custom source (your data
warehouse, a non-canonical CSV, a Polars frame from a feature
pipeline...) the column names won't match the canonical EMIR
field names. The `mapping` dict on `scan_table` reroutes each
canonical field to the user's actual column name — same direction
as the CSV `mapping.fields` pattern in
`opendqi emir scan --mapping mapping.yml`.

Run:
    pip install opendqi
    python examples/python/03_custom_mapping.py
"""
from __future__ import annotations

import json
from pathlib import Path

import opendqi


REPO_ROOT = Path(__file__).resolve().parents[2]
EMIR_TAR_XML = REPO_ROOT / "examples" / "quickstart-emir" / "auth030-tar.xml"


def main() -> None:
    # Get a canonical EMIR Arrow table to start from. In a real
    # pipeline you'd build this from your warehouse / Spark /
    # Polars frame directly.
    table = opendqi.emir.parse_xml(str(EMIR_TAR_XML))

    # Simulate a downstream user who renamed a handful of columns
    # to their internal naming convention.
    rename = {
        "uti":                  "TradeUTI",
        "valuation_timestamp":  "MtmTs",
        "maturity_date":        "ContractEnd",
        "counterparty_1":       "ReportingLEI",
    }
    new_names = [rename.get(n, n) for n in table.column_names]
    user_table = table.rename_columns(new_names)
    print(f"user table has {user_table.num_rows} rows, "
          f"{len(user_table.column_names)} columns including: "
          f"{sorted(rename.values())}")

    # The mapping dict reroutes each canonical EMIR field to the
    # user's column name. Unmapped canonical fields are emitted as
    # None on every record — downstream EMIR.COMP.* checks surface
    # the missingness naturally.
    mapping = {
        # Renamed columns:
        "uti":                  "TradeUTI",
        "valuation_timestamp":  "MtmTs",
        "maturity_date":        "ContractEnd",
        "counterparty_1":       "ReportingLEI",
        # Untouched columns — identity passthrough:
        **{name: name for name in user_table.column_names if name not in rename.values()},
    }

    result = opendqi.emir.scan_table(user_table, mapping)

    print("\n=== summary ===")
    print(json.dumps(result.summary, indent=2, default=str))

    # Quick example: top 5 check IDs by frequency, using Arrow
    # compute (no pandas needed for the analysis).
    print("\n=== top 5 check IDs by frequency ===")
    from collections import Counter
    counts = Counter(result.issues.column("check_id").to_pylist())
    for check_id, n in counts.most_common(5):
        print(f"  {n:>3}× {check_id}")


if __name__ == "__main__":
    main()
