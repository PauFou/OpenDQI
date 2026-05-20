#!/usr/bin/env python3
"""
OpenDQI Python quickstart — Pattern 2: XML → scan_table (no Parquet).

When you already have ISO 20022 XML (`auth.030.001.{03,04}` for
EMIR submissions or `auth.052.001.02` for SFTR), `parse_xml`
produces the canonical Arrow Table in memory; then `scan_table`
runs the standard DQ check suite over it. No Parquet roundtrip.

Useful when:
- You have one-off XML files to audit interactively
- Your data platform already has the XML in memory (e.g. coming
  from an SFTP / TR drop zone listener)
- You don't want to materialise an intermediate Parquet on disk

Run:
    pip install opendqi
    python examples/python/02_parse_xml_then_scan.py
"""
from __future__ import annotations

import json
from pathlib import Path

import opendqi


REPO_ROOT = Path(__file__).resolve().parents[2]
EMIR_TAR_XML = REPO_ROOT / "examples" / "quickstart-emir" / "auth030-tar.xml"


def main() -> None:
    if not EMIR_TAR_XML.exists():
        raise SystemExit(f"✗ fixture missing: {EMIR_TAR_XML}")

    # Parse the EMIR TAR submission XML into a canonical Arrow table.
    # Schema matches `opendqi emir normalize` Parquet output
    # (Decimal128(38,10) / Date32 / Timestamp(μs,UTC) / Utf8).
    table = opendqi.emir.parse_xml(str(EMIR_TAR_XML))
    print(f"parsed {table.num_rows} record(s) from {EMIR_TAR_XML.name}")
    print(f"canonical schema: {len(table.column_names)} columns")

    # Identity mapping: the table already has canonical column names
    # (it came from parse_xml), so the mapping just routes each
    # canonical field to itself.
    mapping = {name: name for name in table.column_names}
    result = opendqi.emir.scan_table(table, mapping)

    print("\n=== summary ===")
    print(json.dumps(result.summary, indent=2, default=str))

    # Drill into issues by severity (pure Arrow, no pandas needed).
    print("\n=== issues by severity ===")
    sev = result.issues.column("severity").to_pylist()
    from collections import Counter
    for severity, n in Counter(sev).most_common():
        print(f"  {severity:>10}: {n}")


if __name__ == "__main__":
    main()
