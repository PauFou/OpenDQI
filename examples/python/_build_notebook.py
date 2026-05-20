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
            "## Where to go next\n"
            "\n"
            "- [`docs/python.md`](../../docs/python.md) — full quickstart with DuckDB / "
            "Polars / pandas / Spark integration patterns\n"
            "- [`docs/python-roadmap.md`](../../docs/python-roadmap.md) — architecture "
            "spec + v0.13+ roadmap\n"
            "- [`README.md`](../../README.md) — project overview, CLI + UI surface, 216 "
            "checks coverage\n"
            "\n"
            "**Status: preview (v0.12.x).** The v1.0 Arrow contract for "
            "`result.issues` (11 cols) is **stable**; the API surface may grow "
            "additively in v0.13 (`scan_directory`, `tr_audit`, Spark UDF namespace)."
        ),
    ]

    NB_PATH.write_text(nbf.writes(nb))
    print(f"✓ wrote unexecuted notebook to {NB_PATH}")
    print(f"  Now run: jupyter nbconvert --to notebook --execute --inplace {NB_PATH}")


if __name__ == "__main__":
    main()
