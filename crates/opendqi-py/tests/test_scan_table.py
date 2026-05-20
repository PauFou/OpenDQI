"""
P4 — `opendqi.{emir,sftr}.scan_table(arrow_tbl, mapping)` +
     `opendqi.{emir,sftr}.parse_xml(path)`.

Validates the Arrow-in surface (zero Parquet roundtrip) and the
new parse_xml helper that turns any firm-submission XML into a
canonical Arrow Table reusable by `scan_table`.
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
def emir_table_from_parquet(tmp_path_factory) -> pa.Table:
    """Generate the EMIR Parquet via the CLI, then load it into a
    pyarrow.Table — that's the typical 'Arrow already in memory'
    starting point of the scan_table API."""
    if not BIN.exists():
        pytest.skip(f"opendqi binary missing at {BIN}")
    if not EMIR_CSV_FIXTURE.exists():
        pytest.skip(f"fixture missing at {EMIR_CSV_FIXTURE}")

    out_dir = tmp_path_factory.mktemp("emir_parquet_p4")
    parquet_path = out_dir / "sample.parquet"
    subprocess.run(
        [
            str(BIN), "emir", "normalize",
            str(EMIR_CSV_FIXTURE),
            "--mapping", str(EMIR_CSV_MAPPING),
            "--out", str(parquet_path),
        ],
        check=True,
        cwd=str(REPO_ROOT),
        env={**os.environ, "RUST_LOG": "warn"},
    )
    return papq.read_table(str(parquet_path))


def test_scan_table_emir_with_identity_mapping(emir_table_from_parquet: pa.Table) -> None:
    """Identity mapping (canonical names already on the table) —
    same scan, same numbers as scan_parquet."""
    import opendqi
    mapping = {name: name for name in emir_table_from_parquet.column_names}
    result = opendqi.emir.scan_table(emir_table_from_parquet, mapping)
    s = result.summary
    # Same numbers as the scan_parquet path (test_scan_parquet.py
    # asserts these against the CLI golden).
    assert s["regime"] == "emir"
    assert s["records_processed"] == 8
    assert s["issues_total"] == 97
    assert 25.0 < s["quality_score"] < 26.0


def test_scan_table_emir_with_renamed_columns(emir_table_from_parquet: pa.Table) -> None:
    """User has columns with non-canonical names + a mapping dict —
    same numbers as the identity case."""
    import opendqi
    # Rename a handful of columns; mapping must reroute correctly.
    table = emir_table_from_parquet
    rename = {"uti": "MyUTI", "valuation_amount": "MTM_Value"}
    new_names = [rename.get(n, n) for n in table.column_names]
    table = table.rename_columns(new_names)
    mapping = {canonical: rename.get(canonical, canonical) for canonical in emir_table_from_parquet.column_names}
    result = opendqi.emir.scan_table(table, mapping)
    s = result.summary
    assert s["records_processed"] == 8
    assert s["issues_total"] == 97  # same — mapping is correct


def test_scan_table_emir_partial_mapping_loses_signals(emir_table_from_parquet: pa.Table) -> None:
    """A mapping that omits the UTI column hides the field on every
    record → triggers `EMIR.COMP.UTI_MISSING` on every record."""
    import opendqi
    mapping = {n: n for n in emir_table_from_parquet.column_names if n != "uti"}
    result = opendqi.emir.scan_table(emir_table_from_parquet, mapping)
    issues = result.issues
    check_ids = issues.column("check_id").to_pylist()
    # 8 records with no uti → 8 UTI_MISSING issues.
    assert check_ids.count("EMIR.COMP.UTI_MISSING") == 8


def test_scan_table_emir_mapping_to_missing_column_raises(emir_table_from_parquet: pa.Table) -> None:
    """validate_mapping_columns turns silent all-None records into
    a loud actionable error."""
    import opendqi
    bad_mapping = {"uti": "ThisColumnDoesNotExist"}
    with pytest.raises(RuntimeError) as exc:
        opendqi.emir.scan_table(emir_table_from_parquet, bad_mapping)
    assert "ThisColumnDoesNotExist" in str(exc.value)


def test_scan_table_emir_accepts_record_batch(emir_table_from_parquet: pa.Table) -> None:
    """Pass a pyarrow.RecordBatch directly (no chunks) — same result."""
    import opendqi
    batches = emir_table_from_parquet.combine_chunks().to_batches()
    assert len(batches) == 1
    rb = batches[0]
    mapping = {name: name for name in emir_table_from_parquet.column_names}
    result = opendqi.emir.scan_table(rb, mapping)
    assert result.summary["records_processed"] == 8


def test_parse_xml_emir_returns_pyarrow_table() -> None:
    """parse_xml on auth.030 yields a canonical-schema pyarrow.Table."""
    import opendqi
    xml = REPO_ROOT / "examples" / "quickstart-emir" / "auth030-tar.xml"
    if not xml.exists():
        pytest.skip(f"fixture missing at {xml}")

    table = opendqi.emir.parse_xml(str(xml))
    assert isinstance(table, pa.Table)
    # The auth030 quickstart fixture contains 5 records (see
    # examples/quickstart-emir/README.md). The exact count is not
    # the contract — non-empty is.
    assert table.num_rows > 0
    # Schema must match the canonical EMIR schema produced by
    # `opendqi emir normalize` (the same `build_emir_batch` is used).
    assert "uti" in table.column_names
    assert "maturity_date" in table.column_names


def test_parse_xml_to_scan_table_round_trip() -> None:
    """parse_xml → scan_table runs the full pipeline without Parquet."""
    import opendqi
    xml = REPO_ROOT / "examples" / "quickstart-emir" / "auth030-tar.xml"
    if not xml.exists():
        pytest.skip(f"fixture missing at {xml}")

    table = opendqi.emir.parse_xml(str(xml))
    mapping = {name: name for name in table.column_names}
    result = opendqi.emir.scan_table(table, mapping)
    assert result.summary["records_processed"] > 0
    # `issues` is a pyarrow.Table per the v1.0 contract.
    assert isinstance(result.issues, pa.Table)


def test_sftr_scan_table_function_present() -> None:
    """SFTR symmetric — exercising end-to-end needs a SFTR parquet."""
    import opendqi
    assert callable(opendqi.sftr.scan_table)
    assert callable(opendqi.sftr.parse_xml)
