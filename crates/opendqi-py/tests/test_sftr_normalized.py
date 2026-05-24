"""
v0.17 H6 — `result.normalized` on the SFTR side.

When `normalize=True` is passed to `opendqi.sftr.{scan_parquet,
scan_table}`, the result's `normalized` attribute is the
canonical SFTR Arrow Table — same schema as the SFTR Parquet
writer's schema. Without the flag, `normalized` stays `None`.

Mirror of test_normalized.py for the SFTR side.
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
    if not BIN.exists():
        pytest.skip(f"opendqi binary missing at {BIN}")
    if not SFTR_XML_FIXTURE.exists():
        pytest.skip(f"fixture missing at {SFTR_XML_FIXTURE}")

    out = tmp_path_factory.mktemp("sftr_norm")
    parquet = out / "sample.parquet"
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
    return parquet


def test_normalized_default_is_none(sftr_parquet: Path) -> None:
    """Without `normalize=True`, result.normalized stays None."""
    import opendqi

    result = opendqi.sftr.scan_parquet(str(sftr_parquet))
    assert result.normalized is None


def test_normalized_true_returns_pyarrow_table(sftr_parquet: Path) -> None:
    import opendqi

    result = opendqi.sftr.scan_parquet(str(sftr_parquet), normalize=True)
    assert result.normalized is not None
    assert isinstance(result.normalized, pa.Table)


def test_normalized_schema_matches_parquet(sftr_parquet: Path) -> None:
    """The normalized Arrow Table's schema MUST match what
    `opendqi_io::sftr_schema()` writes — verified by comparing
    against the Parquet file read directly."""
    import opendqi

    result = opendqi.sftr.scan_parquet(str(sftr_parquet), normalize=True)
    direct = papq.read_table(str(sftr_parquet))
    assert result.normalized.column_names == direct.column_names
    for col in result.normalized.column_names:
        norm_type = result.normalized.schema.field(col).type
        direct_type = direct.schema.field(col).type
        assert norm_type == direct_type, (
            f"SFTR column {col!r} type diverges: normalized={norm_type} "
            f"vs parquet={direct_type}"
        )


def test_normalized_row_count_matches_records(sftr_parquet: Path) -> None:
    """SFTR sample.xml has 10 records ; the normalized Arrow Table
    must carry exactly that many rows."""
    import opendqi

    result = opendqi.sftr.scan_parquet(str(sftr_parquet), normalize=True)
    assert (
        result.normalized.num_rows
        == result.summary["records_processed"]
        == 10
    )


def test_normalized_via_scan_table_too(sftr_parquet: Path) -> None:
    """scan_table also honours `normalize=True`."""
    import opendqi

    table = papq.read_table(str(sftr_parquet))
    mapping = {n: n for n in table.column_names}
    result = opendqi.sftr.scan_table(table, mapping, normalize=True)
    assert result.normalized is not None
    assert result.normalized.num_rows == 10
