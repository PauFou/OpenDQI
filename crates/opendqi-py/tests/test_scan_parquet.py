"""
P2 — `opendqi.{emir,sftr}.scan_parquet(path)` returns a
`PyScanResult` whose `.summary` mirrors `summary.json`.

These tests need a real `.parquet` fixture, generated on the fly by
invoking the workspace `opendqi` debug binary with the
`normalize` subcommand over the synthetic XML fixtures shipped in
`examples/quickstart-emir/`. The binary is expected at
`target/debug/opendqi` from a prior `cargo build`. Tests SKIP if
the binary is missing (e.g. in a clean checkout without a Rust
build) so the harness does not require a Rust toolchain on the
Python CI runner.
"""
from __future__ import annotations

import os
import subprocess
from pathlib import Path

import pytest


REPO_ROOT = Path(__file__).resolve().parents[3]
BIN = REPO_ROOT / "target" / "debug" / "opendqi"
# auth.107 is a TSR (TR output, not a firm submission) — it is the
# input for `emir tr-state-scan`, NOT `emir normalize`. The
# canonical EMIR submission fixture is `examples/emir/sample.csv`
# (+ mapping YAML), which is what the existing `emir-scan-csv`
# golden uses (8 records → 97 issues, quality_score 25.75).
EMIR_CSV_FIXTURE = REPO_ROOT / "examples" / "emir" / "sample.csv"
EMIR_CSV_MAPPING = REPO_ROOT / "examples" / "emir" / "sample_mapping.yml"


@pytest.fixture(scope="session")
def emir_parquet(tmp_path_factory) -> Path:
    """Generate a canonical EMIR Parquet via `opendqi emir normalize`."""
    if not BIN.exists():
        pytest.skip(f"opendqi binary missing at {BIN} (cargo build needed)")
    if not EMIR_CSV_FIXTURE.exists():
        pytest.skip(f"fixture missing at {EMIR_CSV_FIXTURE}")

    out_dir = tmp_path_factory.mktemp("emir_parquet_fixture")
    out_path = out_dir / "sample.parquet"
    subprocess.run(
        [
            str(BIN),
            "emir",
            "normalize",
            str(EMIR_CSV_FIXTURE),
            "--mapping",
            str(EMIR_CSV_MAPPING),
            "--out",
            str(out_path),
        ],
        check=True,
        cwd=str(REPO_ROOT),
        env={**os.environ, "RUST_LOG": "warn"},
    )
    assert out_path.exists(), "normalize did not produce the expected parquet"
    return out_path


def test_scan_parquet_emir_returns_result(emir_parquet: Path) -> None:
    import opendqi
    result = opendqi.emir.scan_parquet(str(emir_parquet))
    assert result is not None
    # repr is informative — exercise it.
    r = repr(result)
    assert "PyScanResult" in r
    assert "regime=" in r


def test_scan_parquet_emir_summary_shape(emir_parquet: Path) -> None:
    """`summary` mirrors `summary.json` (M0.21 IssueAggregator contract)."""
    import opendqi
    result = opendqi.emir.scan_parquet(str(emir_parquet))
    s = result.summary
    # Same 9-field shape as summary.json.
    expected_keys = {
        "regime",
        "files_processed",
        "records_processed",
        "issues_total",
        "issues_by_severity",
        "issues_by_dimension",
        "quality_score",
        "started_at",
        "finished_at",
    }
    assert set(s.keys()) == expected_keys, (
        f"summary keys diverge: missing={expected_keys - set(s.keys())}, "
        f"extra={set(s.keys()) - expected_keys}"
    )
    assert s["regime"] == "emir"
    assert s["files_processed"] == 1
    # `examples/emir/sample.csv` has 8 records (golden-pinned in
    # crates/opendqi-cli/tests/golden/emir-scan-csv.summary.json).
    assert s["records_processed"] == 8
    # …and produces 97 issues at score 25.75. The Python path
    # uses `default_checks()` (89 EMIR single-batch checks), same
    # registry as `opendqi emir scan` — counts must match.
    assert s["issues_total"] == 97
    assert isinstance(s["issues_by_severity"], dict)
    assert isinstance(s["issues_by_dimension"], dict)
    assert isinstance(s["quality_score"], float)
    assert 25.0 < s["quality_score"] < 26.0


def test_scan_parquet_emir_issues_still_none_in_p2(emir_parquet: Path) -> None:
    """P3 wires `result.issues` to a pyarrow.Table; P2 returns None."""
    import opendqi
    result = opendqi.emir.scan_parquet(str(emir_parquet))
    assert result.issues is None
    assert result.normalized is None


def test_scan_parquet_emir_missing_file_raises() -> None:
    """A non-existent path surfaces as a Python error, not a crash."""
    import opendqi
    with pytest.raises(RuntimeError):
        opendqi.emir.scan_parquet("/nonexistent/path/to/some.parquet")


def test_sftr_scan_parquet_function_present() -> None:
    """SFTR symmetric — exercising it end-to-end needs a SFTR parquet
    which is more involved to generate; here we just verify the
    function exists on the submodule."""
    import opendqi
    assert hasattr(opendqi.sftr, "scan_parquet"), (
        "opendqi.sftr.scan_parquet should exist (P2)"
    )
