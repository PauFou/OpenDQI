#!/usr/bin/env python3
"""
One-shot builder for examples/python/quickstart.ipynb.

This script is used at authoring time only (not committed-runnable
as a user-facing example). It constructs the notebook structure
via nbformat, writes the unexecuted .ipynb, and exits.

Then the developer runs:

    jupyter nbconvert --to notebook --execute --inplace \\
        examples/python/quickstart.ipynb

to capture outputs for the committed file.

Remove this script before commit if desired — keeping it makes the
notebook reproducible without hand-editing JSON.
"""
from __future__ import annotations

from pathlib import Path

import nbformat as nbf


HERE = Path(__file__).resolve().parent
NB_PATH = HERE / "quickstart.ipynb"


def md(src: str) -> dict:
    return nbf.v4.new_markdown_cell(src)


def code(src: str) -> dict:
    return nbf.v4.new_code_cell(src)


def main() -> None:
    nb = nbf.v4.new_notebook()
    nb.metadata = {
        "kernelspec": {
            "display_name": "Python 3",
            "language": "python",
            "name": "python3",
        },
        "language_info": {
            "name": "python",
            "version": "3.12.3",
        },
    }

    nb.cells = [
        md(
            "# OpenDQI Python quickstart\n"
            "\n"
            "Three patterns in 3 minutes — same as `examples/python/{01,02,03}_*.py`, "
            "but executable cell-by-cell.\n"
            "\n"
            "**Install:** `pip install opendqi` (v0.12.1+).\n"
            "\n"
            "**This notebook expects** the OpenDQI repo cloned locally so the synthetic "
            "fixtures under `examples/` are reachable. For a stand-alone tutorial that "
            "downloads its own fixtures, see `docs/python.md`."
        ),
        code(
            "import json, os, shutil, subprocess\n"
            "from pathlib import Path\n"
            "\n"
            "import opendqi\n"
            "print('opendqi version:', opendqi.__version__)\n"
            "\n"
            "# Repo root (notebook is at examples/python/quickstart.ipynb)\n"
            "REPO_ROOT = Path.cwd()\n"
            "while not (REPO_ROOT / 'Cargo.toml').exists() and REPO_ROOT != REPO_ROOT.parent:\n"
            "    REPO_ROOT = REPO_ROOT.parent\n"
            "print('repo root:', REPO_ROOT)"
        ),
        md(
            "## Pattern 1 — Scan a normalized Parquet\n"
            "\n"
            "The simplest entry point: hand `scan_parquet` a path to a canonical "
            "EMIR Parquet (produced by `opendqi emir normalize`). Get back a "
            "`summary` dict and an `issues` pyarrow.Table.\n"
            "\n"
            "Here we generate the Parquet on the fly via the CLI binary."
        ),
        code(
            "csv = REPO_ROOT / 'examples' / 'emir' / 'sample.csv'\n"
            "mapping = REPO_ROOT / 'examples' / 'emir' / 'sample_mapping.yml'\n"
            "parquet = REPO_ROOT / 'target' / 'examples' / 'sample.parquet'\n"
            "parquet.parent.mkdir(parents=True, exist_ok=True)\n"
            "\n"
            "if not parquet.exists():\n"
            "    bin_path = shutil.which('opendqi') or str(REPO_ROOT / 'target' / 'debug' / 'opendqi')\n"
            "    subprocess.run(\n"
            "        [bin_path, 'emir', 'normalize', str(csv),\n"
            "         '--mapping', str(mapping), '--out', str(parquet)],\n"
            "        check=True, env={**os.environ, 'RUST_LOG': 'warn'},\n"
            "    )\n"
            "\n"
            "result = opendqi.emir.scan_parquet(str(parquet))\n"
            "print(json.dumps(result.summary, indent=2, default=str))"
        ),
        md(
            "## Pattern 2 — XML → `scan_table` (no Parquet roundtrip)\n"
            "\n"
            "When you already have ISO 20022 XML in memory, `parse_xml` produces "
            "the canonical Arrow Table directly; `scan_table` then runs the same "
            "check suite as `scan_parquet`."
        ),
        code(
            "xml = REPO_ROOT / 'examples' / 'quickstart-emir' / 'auth030-tar.xml'\n"
            "\n"
            "table = opendqi.emir.parse_xml(str(xml))\n"
            "print(f'parsed {table.num_rows} record(s), {len(table.column_names)} columns')\n"
            "\n"
            "mapping = {n: n for n in table.column_names}   # identity\n"
            "result = opendqi.emir.scan_table(table, mapping)\n"
            "print(json.dumps(result.summary, indent=2, default=str))"
        ),
        md(
            "## Pattern 3 — Custom column mapping (the warehouse path)\n"
            "\n"
            "When your Arrow table comes from a custom source, the column names "
            "won't match the canonical EMIR field names. The `mapping` dict reroutes "
            "each canonical field to your actual column name.\n"
            "\n"
            "Below: a small Arrow groupby on `result.issues` showing the top 5 "
            "check IDs — pure Arrow, no pandas needed."
        ),
        code(
            "rename = {\n"
            "    'uti':                 'TradeUTI',\n"
            "    'valuation_timestamp': 'MtmTs',\n"
            "    'maturity_date':       'ContractEnd',\n"
            "}\n"
            "user_table = table.rename_columns([rename.get(n, n) for n in table.column_names])\n"
            "\n"
            "mapping = {\n"
            "    **{name: name for name in user_table.column_names if name not in rename.values()},\n"
            "    'uti':                 'TradeUTI',\n"
            "    'valuation_timestamp': 'MtmTs',\n"
            "    'maturity_date':       'ContractEnd',\n"
            "}\n"
            "result = opendqi.emir.scan_table(user_table, mapping)\n"
            "print(f'records={result.summary[\"records_processed\"]} '\n"
            "      f'issues={result.summary[\"issues_total\"]} '\n"
            "      f'score={result.summary[\"quality_score\"]:.2f}')\n"
            "\n"
            "from collections import Counter\n"
            "top_checks = Counter(result.issues.column('check_id').to_pylist()).most_common(5)\n"
            "print('\\nTop 5 check IDs:')\n"
            "for check_id, n in top_checks:\n"
            "    print(f'  {n:>3}x {check_id}')"
        ),
        md(
            "## Pattern 7 — EMIR Data Quality Pack (24 indicators)\n"
            "\n"
            "The headline DQI feature. Above the 216 granular checks sits "
            "an **aggregated layer** with 24 regulator-style EMIR indicators "
            "(numerator / denominator / rate / threshold / status) + drill-down "
            "evidence. Same scan, two views — committee-readable on top, "
            "forensic underneath. v0.16 grew this from 10 → 24 EMIR indicators "
            "(+ 4 SFTR — see Pattern 8 below).\n"
            "\n"
            "Run on the 5-layer kit at `examples/emir-data-quality-pack/`. The "
            "result has 4 fields:\n"
            "- `result.indicators` — pyarrow.Table, 24 rows × 11 cols (v1.0 stable)\n"
            "- `result.evidence`   — pyarrow.Table, ≤ 480 rows × 7 cols (v1.0 stable)\n"
            "- `result.issues`     — pyarrow.Table, granular (same contract as v0.12+)\n"
            "- `result.summary`    — dict, same shape as `summary.json`\n"
            "\n"
            "**Disclaimer**: DQI verdicts are **internal data quality indicators**, "
            "not regulatory verdicts. A `red` status is an internal alert, not "
            "a declaration of non-compliance — see "
            "[`docs/data-quality-pack.md`](../../docs/data-quality-pack.md)."
        ),
        code(
            "kit = REPO_ROOT / 'examples' / 'emir-data-quality-pack'\n"
            "\n"
            "pack = opendqi.emir.data_quality_pack(\n"
            "    tsr=str(kit / 'tsr.xml'),\n"
            "    tar=str(kit / 'tar.xml'),\n"
            "    msr=str(kit / 'msr.xml'),\n"
            "    mar=str(kit / 'mar.xml'),\n"
            "    feedback=str(kit / 'feedback.xml'),\n"
            "    as_of='2026-05-21',\n"
            ")\n"
            "print(repr(pack))\n"
            "\n"
            "# 24 EMIR indicators — committee-readable\n"
            "ind_df = pack.indicators.select(\n"
            "    ['indicator_id', 'status', 'numerator', 'denominator', 'rate']\n"
            ").to_pandas()\n"
            "ind_df['rate'] = ind_df['rate'].apply(\n"
            "    lambda r: f'{r * 100:.2f}%' if r is not None else ''\n"
            ")\n"
            "print('\\n=== indicators ===')\n"
            "print(ind_df.to_string(index=False))\n"
            "\n"
            "# Drill-down evidence — top offenders\n"
            "print(f'\\n=== evidence ({pack.evidence.num_rows} rows) ===')\n"
            "print(pack.evidence.select(['indicator_id', 'uti', 'explanation']).to_pandas().head(8).to_string(index=False))"
        ),
        md(
            "## Pattern 8 — SFTR Data Quality Pack (v0.16+, 4 indicators)\n"
            "\n"
            "Sister API on the SFTR side. v0.16 ships **4 SFTR indicators** on "
            "the T2 layer of `auth.079` (TSR) + `auth.052` (TAR) :\n"
            "- `DQI_COLLATERAL_VALUE_MISSING_SFTR` (completeness)\n"
            "- `DQI_LOAN_VALUE_MISSING_SFTR` (completeness)\n"
            "- `DQI_LOAN_VALUE_STALE_SFTR` (timeliness — TARGET2 business days)\n"
            "- `DQI_TIM_REPORTING_LATE_SFTR` (timeliness)\n"
            "\n"
            "**Paths-only in v0.16** (no pyarrow.Table dual-input on the SFTR "
            "side yet — v0.17). T3 margin / `auth.080` reconciliation / "
            "`auth.083` missing-collateral indicators are scheduled for v0.17 "
            "too. Same `PyDqiPackResult` shape as Pattern 7 — same 4 fields, "
            "same v1.0 Arrow schemas."
        ),
        code(
            "sftr_kit = REPO_ROOT / 'examples' / 'sftr-data-quality-pack'\n"
            "\n"
            "sftr_pack = opendqi.sftr.data_quality_pack(\n"
            "    tsr=str(sftr_kit / 'tsr.xml'),\n"
            "    tar=str(sftr_kit / 'tar.xml'),\n"
            "    as_of='2026-05-21',\n"
            ")\n"
            "print(repr(sftr_pack))\n"
            "\n"
            "# 4 SFTR indicators\n"
            "sftr_df = sftr_pack.indicators.select(\n"
            "    ['indicator_id', 'status', 'numerator', 'denominator', 'rate']\n"
            ").to_pandas()\n"
            "sftr_df['rate'] = sftr_df['rate'].apply(\n"
            "    lambda r: f'{r * 100:.2f}%' if r is not None else ''\n"
            ")\n"
            "print('\\n=== SFTR indicators ===')\n"
            "print(sftr_df.to_string(index=False))"
        ),
        md(
            "## Where to go next\n"
            "\n"
            "- [`docs/data-quality-pack.md`](../../docs/data-quality-pack.md) — "
            "full DQI spec (24 EMIR + 4 SFTR indicators, thresholds, Arrow schemas, disclaimer)\n"
            "- [`docs/python.md`](../../docs/python.md) — full quickstart with DuckDB / "
            "Polars / pandas / Spark integration patterns\n"
            "- [`docs/python-roadmap.md`](../../docs/python-roadmap.md) — architecture "
            "spec + v0.13+ roadmap\n"
            "- [`README.md`](../../README.md) — project overview, CLI + UI surface, 216 "
            "checks coverage + the v0.15 Data Quality Pack\n"
            "\n"
            "**Status: preview (v0.16.x).** The v1.0 Arrow contracts for "
            "`result.issues` (11 cols, locked v0.12.0), `result.indicators` "
            "(11 cols, locked v0.15.0), and `result.evidence` (7 cols, locked "
            "v0.15.0) are **stable**; v0.16 expanded the DQI surface from 10 → "
            "28 indicators by adding *rows* only, never new columns. The API may "
            "grow additively in v0.17 (SFTR T3 / reconciliation / missing-collateral "
            "indicators, CLI/Python wiring of auth.091 cross-CP DQIs, dual-input "
            "pyarrow.Table on the SFTR side, threshold profile presets)."
        ),
    ]

    NB_PATH.write_text(nbf.writes(nb))
    print(f"✓ wrote unexecuted notebook to {NB_PATH}")
    print(f"  Now run: jupyter nbconvert --to notebook --execute --inplace {NB_PATH}")


if __name__ == "__main__":
    main()
