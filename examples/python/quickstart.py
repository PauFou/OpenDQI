#!/usr/bin/env python3
"""
OpenDQI Python quickstart — the 5-line "does it work?" check.

Three lines that turn an EMIR `auth.030` submission XML into a
real DQ scan, summary dict, and `pyarrow.Table` of issues.

Run from the repo root after `pip install opendqi`:

    python examples/python/quickstart.py

For more detailed patterns (path-based scan on Parquet, custom
column mapping for warehouse-named tables, etc.) see
01_/02_/03_*.py in this directory and `docs/python.md`.
"""
import opendqi

table = opendqi.emir.parse_xml("examples/quickstart-emir/auth030-tar.xml")
result = opendqi.emir.scan_table(
    table,
    mapping={name: name for name in table.column_names},
)

print(result.summary)
print(result.issues.to_pandas().head())
