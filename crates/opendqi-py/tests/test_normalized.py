"""
P5 — `result.normalized` is populated when `normalize=True` is
passed to `scan_parquet` / `scan_table`. Returns the canonical
EMIR/SFTR Arrow Table — same schema as `opendqi_io::{emir,sftr}
_schema()` (the Parquet writer's schema).
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
EMIR_CSV_FIXTURE = REPO_ROOT / "examples" / "emir" / "sample.csv"
EMIR_CSV_MAPPING = REPO_ROOT / "examples" / "emir" / "sample_mapping.yml"


@pytest.fixture(scope="session")
def emir_parquet(tmp_path_factory) -> Path:
    if not BIN.exists():
        pytest.skip(f"opendqi binary missing at {BIN}")
    out = tmp_path_factory.mktemp("emir_norm")
    parquet = out / "sample.parquet"
    subprocess.run(
        [str(BIN), "emir", "normalize",
         str(EMIR_CSV_FIXTURE), "--mapping", str(EMIR_CSV_MAPPING),
         "--out", str(parquet)],
        check=True, cwd=str(REPO_ROOT),
        env={**os.environ, "RUST_LOG": "warn"},
    )
    return parquet


def test_normalized_default_is_none(emir_parquet: Path) -> None:
    """Without `normalize=True`, result.normalized stays None."""
    import opendqi
    result = opendqi.emir.scan_parquet(str(emir_parquet))
    assert result.normalized is None


def test_normalized_true_returns_pyarrow_table(emir_parquet: Path) -> None:
    import opendqi
    result = opendqi.emir.scan_parquet(str(emir_parquet), normalize=True)
    assert result.normalized is not None
    assert isinstance(result.normalized, pa.Table)


def test_normalized_schema_matches_parquet(emir_parquet: Path) -> None:
    """The normalized Arrow Table's schema MUST match what
    `opendqi_io::emir_schema()` writes — verified by comparing
    against the Parquet file read directly."""
    import opendqi
    result = opendqi.emir.scan_parquet(str(emir_parquet), normalize=True)
    direct = papq.read_table(str(emir_parquet))
    # Same column names, in the same order.
    assert result.normalized.column_names == direct.column_names
    # Same types.
    for col in result.normalized.column_names:
        norm_type = result.normalized.schema.field(col).type
        direct_type = direct.schema.field(col).type
        assert norm_type == direct_type, (
            f"column {col!r} type diverges: normalized={norm_type} "
            f"vs parquet={direct_type}"
        )


def test_normalized_row_count_matches_records(emir_parquet: Path) -> None:
    import opendqi
    result = opendqi.emir.scan_parquet(str(emir_parquet), normalize=True)
    assert result.normalized.num_rows == result.summary["records_processed"] == 8


def test_normalized_via_scan_table_too(emir_parquet: Path) -> None:
    """scan_table also honours `normalize=True`."""
    import opendqi
    table = papq.read_table(str(emir_parquet))
    mapping = {n: n for n in table.column_names}
    result = opendqi.emir.scan_table(table, mapping, normalize=True)
    assert result.normalized is not None
    # Round-trip should preserve row count.
    assert result.normalized.num_rows == 8
