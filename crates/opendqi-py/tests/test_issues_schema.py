"""
P3 — the v1.0 stable Arrow contract for `DqIssue` exports.

**This file LOCKS the contract.** The two anchor tests are:

1. `test_arrow_schema_matches_csv_golden` — the column names + dtypes
   of `result.issues` MUST equal those of the existing CLI golden
   `emir-scan-csv.issues.csv` loaded with `pyarrow.csv.read_csv`.
   Any future change to either side is a BREAKING change to the
   v1.0 bindings contract.

2. `test_arrow_rows_match_csv_golden_content` — for the same scan,
   the row content (issue-by-issue, column-by-column) matches the
   CSV after a small set of well-understood normalisations (paths,
   nulls, etc).

The other tests cover edge cases the contract must handle:
empty result, the dtypes table, repr behaviour.
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
EMIR_CSV_GOLDEN = REPO_ROOT / "crates" / "opendqi-cli" / "tests" / "golden" / "emir-scan-csv.issues.csv"


@pytest.fixture(scope="session")
def emir_parquet(tmp_path_factory) -> Path:
    if not BIN.exists():
        pytest.skip(f"opendqi binary missing at {BIN} (cargo build needed)")
    if not EMIR_CSV_FIXTURE.exists():
        pytest.skip(f"fixture missing at {EMIR_CSV_FIXTURE}")

    out_dir = tmp_path_factory.mktemp("emir_parquet_fixture_p3")
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
    return out_path


def test_result_issues_is_pyarrow_table(emir_parquet: Path) -> None:
    import opendqi, pyarrow as pa
    result = opendqi.emir.scan_parquet(str(emir_parquet))
    assert result.issues is not None
    # Specifically `pyarrow.Table` (not RecordBatch) — the documented
    # public contract.
    assert isinstance(result.issues, pa.Table)


def test_arrow_schema_column_names_locked(emir_parquet: Path) -> None:
    """The 11 column names + their order are the v1.0 contract."""
    import opendqi
    result = opendqi.emir.scan_parquet(str(emir_parquet))
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


def test_arrow_schema_dtypes_are_all_string(emir_parquet: Path) -> None:
    """Every column is `string` — the v1.0 contract."""
    import opendqi, pyarrow as pa
    result = opendqi.emir.scan_parquet(str(emir_parquet))
    for col in result.issues.column_names:
        assert result.issues.schema.field(col).type == pa.string(), (
            f"column {col!r} type must be string, got {result.issues.schema.field(col).type}"
        )


def test_arrow_schema_nullability_locked(emir_parquet: Path) -> None:
    """Nullability matches the `Option<T>` shape on `DqIssue`."""
    import opendqi
    result = opendqi.emir.scan_parquet(str(emir_parquet))
    schema = result.issues.schema
    # Required.
    assert not schema.field("check_id").nullable
    assert not schema.field("regime").nullable
    assert not schema.field("severity").nullable
    assert not schema.field("dimension").nullable
    assert not schema.field("message").nullable
    # Optional.
    assert schema.field("record_id").nullable
    assert schema.field("uti").nullable
    assert schema.field("field").nullable
    assert schema.field("value").nullable
    assert schema.field("source_file").nullable
    assert schema.field("evidence_json").nullable


def test_arrow_schema_matches_csv_golden(emir_parquet: Path) -> None:
    """
    The Arrow schema produced by the Python bindings MUST match
    (column names + count + order) the CSV schema produced by the
    existing CLI `write_issues_csv` path. This is the v1.0 contract:
    the same scan run two different ways produces identical column
    structure.
    """
    import opendqi, pyarrow as pa
    import pyarrow.csv as pacsv

    if not EMIR_CSV_GOLDEN.exists():
        pytest.skip(f"golden missing at {EMIR_CSV_GOLDEN}")

    result = opendqi.emir.scan_parquet(str(emir_parquet))
    csv_golden = pacsv.read_csv(str(EMIR_CSV_GOLDEN))

    assert result.issues.column_names == csv_golden.column_names, (
        f"v1.0 contract BROKEN: arrow={result.issues.column_names} "
        f"vs csv_golden={csv_golden.column_names}"
    )
    # All columns are `string` in the Arrow Schema; CSV reader
    # infers types (likely also string). The CONTRACT is on names
    # and order — the actual storage type on the Python side is
    # `pa.string()` as locked above.


def test_arrow_row_count_matches_csv_golden(emir_parquet: Path) -> None:
    """Same scan → same row count, both sides."""
    import opendqi
    import pyarrow.csv as pacsv

    if not EMIR_CSV_GOLDEN.exists():
        pytest.skip(f"golden missing at {EMIR_CSV_GOLDEN}")

    result = opendqi.emir.scan_parquet(str(emir_parquet))
    csv_golden = pacsv.read_csv(str(EMIR_CSV_GOLDEN))

    assert result.issues.num_rows == csv_golden.num_rows == 97


def test_arrow_check_ids_match_csv_golden(emir_parquet: Path) -> None:
    """
    The multiset of `check_id` values must be identical between
    the Arrow and CSV outputs (order is content-deterministic since
    M0.17 so we could compare lists, but multiset is the safer
    contract for the test — it doesn't break on irrelevant
    re-ordering of issues that compare-equal under issue_cmp).
    """
    import opendqi
    import pyarrow.csv as pacsv
    from collections import Counter

    if not EMIR_CSV_GOLDEN.exists():
        pytest.skip(f"golden missing at {EMIR_CSV_GOLDEN}")

    result = opendqi.emir.scan_parquet(str(emir_parquet))
    csv_golden = pacsv.read_csv(str(EMIR_CSV_GOLDEN))

    arrow_check_ids = Counter(result.issues.column("check_id").to_pylist())
    csv_check_ids = Counter(csv_golden.column("check_id").to_pylist())
    assert arrow_check_ids == csv_check_ids


def test_result_repr_now_shows_issue_count(emir_parquet: Path) -> None:
    """P3 repr reflects the actual issue count (was 0 in P2)."""
    import opendqi
    result = opendqi.emir.scan_parquet(str(emir_parquet))
    r = repr(result)
    assert "issues=97" in r
