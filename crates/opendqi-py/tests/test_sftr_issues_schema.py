"""
v0.17 H7 — v1.0 stable Arrow contract for `DqIssue` exports
on the SFTR side.

Mirror of test_issues_schema.py for the SFTR side. Locks the
contract that the 11-column issues schema is **regime-agnostic** :
SFTR and EMIR scans produce identical column names + types +
nullability. The schema is the same; only the regime-tagged
content differs.
"""
from __future__ import annotations

import os
import subprocess
from pathlib import Path

import pytest


REPO_ROOT = Path(__file__).resolve().parents[3]
BIN = REPO_ROOT / "target" / "debug" / "opendqi"
SFTR_XML_FIXTURE = REPO_ROOT / "examples" / "sftr" / "iso20022" / "sample.xml"
SFTR_CSV_GOLDEN = (
    REPO_ROOT
    / "crates"
    / "opendqi-cli"
    / "tests"
    / "golden"
    / "sftr-scan.issues.csv"
)


@pytest.fixture(scope="session")
def sftr_parquet(tmp_path_factory) -> Path:
    if not BIN.exists():
        pytest.skip(f"opendqi binary missing at {BIN} (cargo build needed)")
    if not SFTR_XML_FIXTURE.exists():
        pytest.skip(f"fixture missing at {SFTR_XML_FIXTURE}")

    out_dir = tmp_path_factory.mktemp("sftr_parquet_fixture_h7")
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
    return out_path


def test_result_issues_is_pyarrow_table(sftr_parquet: Path) -> None:
    import opendqi
    import pyarrow as pa

    result = opendqi.sftr.scan_parquet(str(sftr_parquet))
    assert result.issues is not None
    assert isinstance(result.issues, pa.Table)


def test_arrow_schema_column_names_locked(sftr_parquet: Path) -> None:
    """The 11 column names + their order — identical to the EMIR
    side (regime-agnostic schema)."""
    import opendqi

    result = opendqi.sftr.scan_parquet(str(sftr_parquet))
    expected = [
        "check_id",
        "regime",
        "severity",
        "dimension",
        "record_id",
        "uti",
        "field",
        "value",
        "message",
        "source_file",
        "evidence_json",
    ]
    assert result.issues.column_names == expected


def test_arrow_schema_dtypes_are_all_string(sftr_parquet: Path) -> None:
    """Every column is `string` — same v1.0 contract as EMIR."""
    import opendqi
    import pyarrow as pa

    result = opendqi.sftr.scan_parquet(str(sftr_parquet))
    for col in result.issues.column_names:
        assert result.issues.schema.field(col).type == pa.string(), (
            f"SFTR column {col!r} type must be string, got "
            f"{result.issues.schema.field(col).type}"
        )


def test_arrow_schema_nullability_locked(sftr_parquet: Path) -> None:
    """Nullability matches the Option<T> shape on DqIssue — same
    contract as EMIR (regime-agnostic)."""
    import opendqi

    result = opendqi.sftr.scan_parquet(str(sftr_parquet))
    schema = result.issues.schema
    # Required.
    for col in ("check_id", "regime", "severity", "dimension", "message"):
        assert not schema.field(col).nullable, f"{col} must be required"
    # Optional.
    for col in ("record_id", "uti", "field", "value", "source_file", "evidence_json"):
        assert schema.field(col).nullable, f"{col} must be optional"


def test_regime_column_is_sftr(sftr_parquet: Path) -> None:
    """Every SFTR issue carries regime=='sftr' — proves the regime
    tag is propagated through the pipeline."""
    import opendqi

    result = opendqi.sftr.scan_parquet(str(sftr_parquet))
    regimes = set(result.issues.column("regime").to_pylist())
    assert regimes == {"sftr"}, (
        f"SFTR scan produced non-SFTR regime tags: {regimes}"
    )


def test_arrow_schema_matches_csv_golden(sftr_parquet: Path) -> None:
    """The Arrow schema produced by the Python bindings MUST match
    the CSV schema produced by the existing CLI `write_issues_csv`
    path on the SFTR side too."""
    import opendqi
    import pyarrow.csv as pacsv

    if not SFTR_CSV_GOLDEN.exists():
        pytest.skip(f"golden missing at {SFTR_CSV_GOLDEN}")

    result = opendqi.sftr.scan_parquet(str(sftr_parquet))
    csv_golden = pacsv.read_csv(str(SFTR_CSV_GOLDEN))

    assert result.issues.column_names == csv_golden.column_names, (
        f"v1.0 contract BROKEN on SFTR side: arrow="
        f"{result.issues.column_names} vs csv_golden="
        f"{csv_golden.column_names}"
    )


def test_arrow_row_count_matches_csv_golden(sftr_parquet: Path) -> None:
    """SFTR golden has 27 issues — locked count."""
    import opendqi
    import pyarrow.csv as pacsv

    if not SFTR_CSV_GOLDEN.exists():
        pytest.skip(f"golden missing at {SFTR_CSV_GOLDEN}")

    result = opendqi.sftr.scan_parquet(str(sftr_parquet))
    csv_golden = pacsv.read_csv(str(SFTR_CSV_GOLDEN))
    assert result.issues.num_rows == csv_golden.num_rows == 27


def test_arrow_check_ids_match_csv_golden(sftr_parquet: Path) -> None:
    """Multiset of check_id values identical between Arrow and CSV
    on the SFTR side — same Rust check registry, same content."""
    import opendqi
    import pyarrow.csv as pacsv
    from collections import Counter

    if not SFTR_CSV_GOLDEN.exists():
        pytest.skip(f"golden missing at {SFTR_CSV_GOLDEN}")

    result = opendqi.sftr.scan_parquet(str(sftr_parquet))
    csv_golden = pacsv.read_csv(str(SFTR_CSV_GOLDEN))

    arrow_check_ids = Counter(result.issues.column("check_id").to_pylist())
    csv_check_ids = Counter(csv_golden.column("check_id").to_pylist())
    assert arrow_check_ids == csv_check_ids
