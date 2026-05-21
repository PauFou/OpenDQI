#!/usr/bin/env python3
"""
OpenDQI Python quickstart — Pattern 7 (v0.15.0): EMIR Data
Quality Pack.

The headline v0.15 feature. Aggregates the 5 EMIR TR layers
(TSR / TAR / MSR / MAR / Feedback) into 10 regulator-style
indicators with numerator / denominator / rate / threshold /
status. Each indicator carries ≤ 20 evidence rows for
drill-down.

The granular issue stream (the existing 216 checks) is
**co-produced**, not replaced — `result.issues` is the same
v1.0 11-column pyarrow.Table contract as v0.12+.

This example uses the file-path input (str). For the
pyarrow.Table input (the path you'd take from a Spark /
DuckDB / Polars notebook), see the docs:
https://github.com/PauFou/OpenDQI/blob/main/docs/data-quality-pack.md

Run from the repo root:

    python examples/python/07_data_quality_pack.py
"""
from __future__ import annotations

import json
from pathlib import Path

import opendqi


REPO_ROOT = Path(__file__).resolve().parents[2]
TSR = REPO_ROOT / "examples" / "quickstart-emir" / "auth107-tsr.xml"
TAR = REPO_ROOT / "examples" / "quickstart-emir" / "auth030-tar.xml"
FBK = REPO_ROOT / "examples" / "quickstart-emir" / "auth092-feedback.xml"


def main() -> None:
    for f in (TSR, TAR, FBK):
        if not f.exists():
            raise SystemExit(f"✗ fixture missing: {f}")

    result = opendqi.emir.data_quality_pack(
        tsr=str(TSR),
        tar=str(TAR),
        feedback=str(FBK),
        as_of="2026-05-21",
    )

    print(repr(result))
    print()

    # ----- Aggregated layer -----
    print("=== Data Quality Indicators (10) ===")
    df = result.indicators.select(
        [
            "indicator_id",
            "status",
            "numerator",
            "denominator",
            "rate",
        ]
    ).to_pandas()
    # Format rate as percent for the print
    df["rate"] = df["rate"].apply(
        lambda r: f"{r * 100:.2f}%" if r is not None else ""
    )
    print(df.to_string(index=False))
    print()

    # ----- Drill-down evidence -----
    print(f"=== Evidence ({result.evidence.num_rows} rows) ===")
    ev_df = result.evidence.select(
        ["indicator_id", "uti", "observed_value", "explanation"]
    ).to_pandas()
    print(ev_df.head(10).to_string(index=False))
    print()

    # ----- Granular issues co-produced (the existing 216-check stream) -----
    print(
        f"=== Granular issues co-produced: {result.issues.num_rows} rows "
        f"(v1.0 11-col contract) ==="
    )
    print(f"columns: {result.issues.column_names}")
    print()

    # ----- Summary (same shape as summary.json) -----
    print("=== Summary ===")
    print(json.dumps(result.summary, indent=2, default=str))
    print()

    # ----- One-call disk export -----
    out = Path("/tmp/opendqi-quickstart-dqi-pack")
    result.report(str(out))
    print(f"Wrote 5 artefacts to {out}/")
    print("  report.html, summary.json, issues.csv, indicators.csv, evidence.csv")


if __name__ == "__main__":
    main()
