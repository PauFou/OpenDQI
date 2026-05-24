"""
v0.17 H2 — `opendqi.sftr.{scan_directory, scan_files}`.

SFTR mirror of `test_scan_directory.py`. Builds a mixed
{1 XML + 1 Parquet} fixture directory, exercises the two
multi-file aggregator entry points, and locks the error
behaviours (csv rejected without mapping, empty input lists,
unsupported extensions).

Tests SKIP rather than fail when the workspace `opendqi` debug
binary or the SFTR fixtures are missing.
"""
from __future__ import annotations

import os
import shutil
import subprocess
from pathlib import Path

import pytest


REPO_ROOT = Path(__file__).resolve().parents[3]
BIN = REPO_ROOT / "target" / "debug" / "opendqi"
SFTR_XML_FIXTURE = REPO_ROOT / "examples" / "sftr" / "iso20022" / "sample.xml"


@pytest.fixture(scope="session")
def sftr_mixed_dir(tmp_path_factory) -> Path:
    """A directory containing 1 .xml + 1 .parquet SFTR file."""
    if not BIN.exists():
        pytest.skip(f"opendqi binary missing at {BIN}")
    if not SFTR_XML_FIXTURE.exists():
        pytest.skip(f"fixture missing at {SFTR_XML_FIXTURE}")

    out_dir = tmp_path_factory.mktemp("sftr_mixed_dir")
    # Generate Parquet from the auth.052 XML via the CLI.
    parquet = out_dir / "sample.parquet"
    subprocess.run(
        [
            str(BIN),
            "sftr",
            "normalize",
            str(SFTR_XML_FIXTURE),
            "--out",
            str(parquet),
        ],
        check=True,
        cwd=str(REPO_ROOT),
        env={**os.environ, "RUST_LOG": "warn"},
    )
    # Copy the same XML in alongside so the directory has 2 files
    # processing the same 10 records each way.
    shutil.copy(SFTR_XML_FIXTURE, out_dir / "sample.xml")
    return out_dir


def test_scan_directory_aggregates_mixed_files(sftr_mixed_dir: Path) -> None:
    """1 .xml (10 records) + 1 .parquet (10 records, same records
    round-tripped) = 20 aggregated. We don't assert dedup — the
    aggregator concatenates records by design, leaving dedup to
    downstream consumers."""
    import opendqi

    result = opendqi.sftr.scan_directory(str(sftr_mixed_dir))
    assert result.summary["regime"] == "sftr"
    assert result.summary["files_processed"] == 2
    assert result.summary["records_processed"] == 20
    # Issues strictly > the per-file scan (10 records → 27 issues
    # in test_sftr_scan.py).
    assert result.summary["issues_total"] >= 27
    assert result.issues is not None
    assert result.issues.num_rows == result.summary["issues_total"]


def test_scan_files_explicit_list_matches_scan_directory(sftr_mixed_dir: Path) -> None:
    """scan_files() with an explicit list of 2 paths == scan_directory() on the dir."""
    import opendqi

    paths = sorted(
        str(p) for p in sftr_mixed_dir.iterdir() if p.suffix in (".xml", ".parquet")
    )
    assert len(paths) == 2

    result_files = opendqi.sftr.scan_files(paths)
    result_dir = opendqi.sftr.scan_directory(str(sftr_mixed_dir))

    assert (
        result_files.summary["files_processed"]
        == result_dir.summary["files_processed"]
        == 2
    )
    assert (
        result_files.summary["records_processed"]
        == result_dir.summary["records_processed"]
    )
    assert result_files.summary["issues_total"] == result_dir.summary["issues_total"]


def test_scan_directory_rejects_csv_with_helpful_error(tmp_path: Path) -> None:
    """A directory containing only .csv should error with the mapping hint."""
    import opendqi

    csv_path = tmp_path / "some.csv"
    csv_path.write_text("uti,counterparty_1\nU1,LEI001\n")

    with pytest.raises(ValueError) as exc:
        opendqi.sftr.scan_directory(str(tmp_path))

    msg = str(exc.value)
    assert ".csv" in msg.lower() or "csv" in msg.lower()
    assert "mapping" in msg.lower()
    assert "scan_table" in msg or "pyarrow.csv" in msg


def test_scan_files_empty_list_raises() -> None:
    """Empty input list should be an actionable error."""
    import opendqi

    with pytest.raises(ValueError) as exc:
        opendqi.sftr.scan_files([])
    assert "no input files" in str(exc.value).lower()


def test_scan_directory_unsupported_extension_raises(tmp_path: Path) -> None:
    """A directory with only a .txt file should error clearly."""
    import opendqi

    (tmp_path / "notes.txt").write_text("hello")

    with pytest.raises(ValueError) as exc:
        opendqi.sftr.scan_directory(str(tmp_path))
    assert "no input files" in str(exc.value).lower()


def test_scan_files_single_parquet_matches_scan_parquet(sftr_mixed_dir: Path) -> None:
    """scan_files([single_parquet]) ≡ scan_parquet(single_parquet)."""
    import opendqi

    parquet = sftr_mixed_dir / "sample.parquet"
    r_files = opendqi.sftr.scan_files([str(parquet)])
    r_single = opendqi.sftr.scan_parquet(str(parquet))

    assert (
        r_files.summary["records_processed"]
        == r_single.summary["records_processed"]
    )
    assert r_files.summary["issues_total"] == r_single.summary["issues_total"]
    assert r_files.summary["files_processed"] == r_single.summary["files_processed"] == 1
