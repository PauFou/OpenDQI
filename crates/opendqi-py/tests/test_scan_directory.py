"""
v0.13.0 — `opendqi.{emir,sftr}.scan_directory` + `scan_files`
multi-file aggregator entry points.

Both walk a directory (or take an explicit list of paths),
dispatch `.xml` and `.parquet` to the right reader, aggregate
the records, and run the standard `default_*_checks()` suite
over the union. `.csv` is rejected (requires a column mapping).
"""
from __future__ import annotations

import os
import subprocess
from pathlib import Path

import pytest


REPO_ROOT = Path(__file__).resolve().parents[3]
BIN = REPO_ROOT / "target" / "debug" / "opendqi"
EMIR_CSV_FIXTURE = REPO_ROOT / "examples" / "emir" / "sample.csv"
EMIR_CSV_MAPPING = REPO_ROOT / "examples" / "emir" / "sample_mapping.yml"
EMIR_TAR_XML = REPO_ROOT / "examples" / "quickstart-emir" / "auth030-tar.xml"


@pytest.fixture(scope="session")
def emir_mixed_dir(tmp_path_factory) -> Path:
    """A directory containing 1 .xml + 1 .parquet EMIR file."""
    if not BIN.exists():
        pytest.skip(f"opendqi binary missing at {BIN}")
    if not (EMIR_CSV_FIXTURE.exists() and EMIR_TAR_XML.exists()):
        pytest.skip("fixtures missing")

    out_dir = tmp_path_factory.mktemp("emir_mixed_dir")
    # Generate Parquet from CSV via the CLI.
    parquet = out_dir / "sample.parquet"
    subprocess.run(
        [
            str(BIN), "emir", "normalize",
            str(EMIR_CSV_FIXTURE),
            "--mapping", str(EMIR_CSV_MAPPING),
            "--out", str(parquet),
        ],
        check=True, cwd=str(REPO_ROOT),
        env={**os.environ, "RUST_LOG": "warn"},
    )
    # Copy the XML fixture in alongside.
    import shutil
    shutil.copy(EMIR_TAR_XML, out_dir / "auth030-tar.xml")
    return out_dir


def test_scan_directory_aggregates_mixed_files(emir_mixed_dir: Path) -> None:
    """1 .xml (20 records) + 1 .parquet (8 records) = 28 aggregated."""
    import opendqi
    result = opendqi.emir.scan_directory(str(emir_mixed_dir))
    assert result.summary["files_processed"] == 2
    # 8 from sample.parquet + 20 from auth030-tar.xml
    assert result.summary["records_processed"] == 28
    # Issues should be the union of per-file scans — strictly > either alone.
    assert result.summary["issues_total"] > 100
    # Issues table populated.
    assert result.issues is not None
    assert result.issues.num_rows == result.summary["issues_total"]


def test_scan_files_explicit_list(emir_mixed_dir: Path) -> None:
    """scan_files() with an explicit list of 2 paths == scan_directory() on the dir."""
    import opendqi
    paths = sorted(str(p) for p in emir_mixed_dir.iterdir() if p.suffix in (".xml", ".parquet"))
    assert len(paths) == 2

    result_files = opendqi.emir.scan_files(paths)
    result_dir = opendqi.emir.scan_directory(str(emir_mixed_dir))

    # Same numbers — discover_emir_inputs returns the same set, sorted.
    assert result_files.summary["files_processed"] == result_dir.summary["files_processed"] == 2
    assert result_files.summary["records_processed"] == result_dir.summary["records_processed"]
    assert result_files.summary["issues_total"] == result_dir.summary["issues_total"]


def test_scan_directory_rejects_csv_with_helpful_error(tmp_path: Path) -> None:
    """A directory containing only .csv should error with the mapping hint."""
    import opendqi
    csv_path = tmp_path / "some.csv"
    csv_path.write_text("uti,counterparty_1\nU1,LEI001\n")

    with pytest.raises(ValueError) as exc:
        opendqi.emir.scan_directory(str(tmp_path))

    msg = str(exc.value)
    assert ".csv" in msg.lower() or "csv" in msg.lower()
    assert "mapping" in msg.lower()
    assert "scan_table" in msg or "pyarrow.csv" in msg


def test_scan_files_empty_list_raises(tmp_path: Path) -> None:
    """Empty input list should be an actionable error, not a silent zero-issue scan."""
    import opendqi
    with pytest.raises(ValueError) as exc:
        opendqi.emir.scan_files([])
    assert "no input files" in str(exc.value).lower()


def test_scan_directory_unsupported_extension_raises(tmp_path: Path) -> None:
    """A directory with only a .txt file should error clearly."""
    import opendqi
    (tmp_path / "notes.txt").write_text("hello")

    # discover_emir_inputs filters by extension so empty list, then our
    # scan_*_paths helper raises "no input files". This is the right
    # behaviour: silent ignore would mask config bugs.
    with pytest.raises(ValueError) as exc:
        opendqi.emir.scan_directory(str(tmp_path))
    assert "no input files" in str(exc.value).lower()


def test_sftr_scan_directory_function_present() -> None:
    """SFTR symmetric — exercising end-to-end needs an SFTR fixture dir."""
    import opendqi
    assert callable(opendqi.sftr.scan_directory)
    assert callable(opendqi.sftr.scan_files)


def test_scan_files_single_path_matches_scan_parquet(emir_mixed_dir: Path) -> None:
    """scan_files([single_parquet]) ≡ scan_parquet(single_parquet)."""
    import opendqi
    parquet = emir_mixed_dir / "sample.parquet"
    r_files = opendqi.emir.scan_files([str(parquet)])
    r_single = opendqi.emir.scan_parquet(str(parquet))

    assert r_files.summary["records_processed"] == r_single.summary["records_processed"]
    assert r_files.summary["issues_total"] == r_single.summary["issues_total"]
    # `files_processed` is 1 in both cases.
    assert r_files.summary["files_processed"] == r_single.summary["files_processed"] == 1
