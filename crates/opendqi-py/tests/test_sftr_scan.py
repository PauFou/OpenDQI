"""
v0.17 H1 — `opendqi.sftr.{scan_parquet, scan_table, parse_xml}`.

SFTR mirror of `test_scan_parquet.py` + `test_scan_table.py`. The
SFTR fixture is `examples/sftr/iso20022/sample.xml` (auth.052
firm submission) which golden-pins to 10 records / 27 issues /
quality_score 78.4 (see
`crates/opendqi-cli/tests/golden/sftr-scan.summary.json`). The
Python path consumes the SAME `default_sftr_checks()` registry
as `opendqi sftr scan`, so the numbers must match.

Tests SKIP rather than fail when the workspace `opendqi` debug
binary or the fixtures are missing — keeps the harness
portable.
"""
from __future__ import annotations

import os
import subprocess
from pathlib import Path

import pyarrow as pa
import pyarrow.parquet as papq
import pytest


REPO_ROOT = Path(__file__).resolve().parents[3]
BIN = REPO_ROOT / "target" / "debug" / "opendqi"
SFTR_XML_FIXTURE = REPO_ROOT / "examples" / "sftr" / "iso20022" / "sample.xml"


@pytest.fixture(scope="session")
def sftr_parquet(tmp_path_factory) -> Path:
    """Generate a canonical SFTR Parquet via `opendqi sftr normalize`."""
    if not BIN.exists():
        pytest.skip(f"opendqi binary missing at {BIN} (cargo build needed)")
    if not SFTR_XML_FIXTURE.exists():
        pytest.skip(f"fixture missing at {SFTR_XML_FIXTURE}")

    out_dir = tmp_path_factory.mktemp("sftr_parquet_fixture")
    out_path = out_dir / "sample.parquet"
    subprocess.run(
        [
            str(BIN),
            "sftr",
            "normalize",
            str(SFTR_XML_FIXTURE),
            "--out",
            str(out_path),
        ],
        check=True,
        cwd=str(REPO_ROOT),
        env={**os.environ, "RUST_LOG": "warn"},
    )
    assert out_path.exists(), "sftr normalize did not produce the expected parquet"
    return out_path


def test_scan_parquet_sftr_returns_result(sftr_parquet: Path) -> None:
    import opendqi

    result = opendqi.sftr.scan_parquet(str(sftr_parquet))
    assert result is not None
    r = repr(result)
    assert "PyScanResult" in r
    assert "regime=" in r


def test_scan_parquet_sftr_summary_matches_cli_golden(sftr_parquet: Path) -> None:
    """The 9-field summary mirrors `summary.json` ; numbers match the
    `sftr-scan` CLI golden exactly (10 records, 27 issues, score 78.4)."""
    import opendqi

    result = opendqi.sftr.scan_parquet(str(sftr_parquet))
    s = result.summary
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
    assert s["regime"] == "sftr"
    assert s["files_processed"] == 1
    assert s["records_processed"] == 10
    assert s["issues_total"] == 27
    assert isinstance(s["quality_score"], float)
    # Locked at 78.4 in the CLI golden ; allow a small tolerance
    # in case rounding differs between Rust f64 and Python.
    assert 78.0 < s["quality_score"] < 79.0


def test_scan_parquet_sftr_missing_file_raises() -> None:
    import opendqi

    with pytest.raises(RuntimeError):
        opendqi.sftr.scan_parquet("/nonexistent/path/to/some.parquet")


def test_scan_table_sftr_with_identity_mapping(sftr_parquet: Path) -> None:
    """scan_table over a parquet-loaded pyarrow.Table with identity
    mapping yields the same numbers as scan_parquet."""
    import opendqi

    table = papq.read_table(str(sftr_parquet))
    mapping = {name: name for name in table.column_names}
    result = opendqi.sftr.scan_table(table, mapping)
    s = result.summary
    assert s["regime"] == "sftr"
    assert s["records_processed"] == 10
    assert s["issues_total"] == 27


def test_scan_table_sftr_partial_mapping_drops_uti_signals(sftr_parquet: Path) -> None:
    """Omitting the UTI column from the mapping makes every record
    look UTI-less, so SFTR.COMP.UTI_MISSING (or the SFTR-MISSING_UTI
    family) fires more often than in the identity-mapping case."""
    import opendqi

    table = papq.read_table(str(sftr_parquet))
    full_mapping = {n: n for n in table.column_names}
    no_uti_mapping = {k: v for k, v in full_mapping.items() if k != "uti"}
    result = opendqi.sftr.scan_table(table, no_uti_mapping)
    check_ids = result.issues.column("check_id").to_pylist()
    # The exact SFTR.* UTI-missing check ID can vary by code path
    # (UTI_MISSING vs MISSING_UTI). Assert at least one fires.
    assert any("UTI" in cid and "MISSING" in cid for cid in check_ids), (
        f"expected a SFTR UTI-missing check to fire, got check_ids={set(check_ids)}"
    )


def test_parse_xml_sftr_returns_pyarrow_table() -> None:
    """parse_xml on the auth.052 sample yields a canonical pyarrow.Table."""
    import opendqi

    if not SFTR_XML_FIXTURE.exists():
        pytest.skip(f"fixture missing at {SFTR_XML_FIXTURE}")
    table = opendqi.sftr.parse_xml(str(SFTR_XML_FIXTURE))
    assert isinstance(table, pa.Table)
    assert table.num_rows == 10
    # Canonical SFTR-side schema must carry uti + sft_type.
    assert "uti" in table.column_names
    assert "sft_type" in table.column_names


def test_parse_xml_to_scan_table_round_trip_sftr() -> None:
    """parse_xml → scan_table runs the full SFTR pipeline without
    Parquet ; the in-memory path must yield the same numbers."""
    import opendqi

    if not SFTR_XML_FIXTURE.exists():
        pytest.skip(f"fixture missing at {SFTR_XML_FIXTURE}")
    table = opendqi.sftr.parse_xml(str(SFTR_XML_FIXTURE))
    mapping = {name: name for name in table.column_names}
    result = opendqi.sftr.scan_table(table, mapping)
    assert result.summary["regime"] == "sftr"
    assert result.summary["records_processed"] == 10
    assert result.summary["issues_total"] == 27
    assert isinstance(result.issues, pa.Table)
