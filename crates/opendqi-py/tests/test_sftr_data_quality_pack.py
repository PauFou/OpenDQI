"""
v0.16.0 — `opendqi.sftr.data_quality_pack` Python binding.

Locks the v1.0 stable Arrow contracts for `result.indicators`
and `result.evidence` against the CLI golden — the same scan run
two different ways (CLI + Python) must produce identical column
structure for the 4 SFTR indicators v0.16 ships.

v0.16 is **paths-only** on the SFTR DQI pack — TSR/TAR Arrow
converters arrive in v0.17 alongside the T3-layer indicators
and the reconciliation/missing-collateral computers.
"""
from __future__ import annotations

from pathlib import Path

import pytest


REPO_ROOT = Path(__file__).resolve().parents[3]
TSR_FIXTURE = REPO_ROOT / "examples" / "sftr" / "tr_state" / "auth079-sample.xml"
TAR_FIXTURE = REPO_ROOT / "examples" / "sftr" / "tr_activity" / "auth052-tar-sample.xml"

INDICATORS_GOLDEN = (
    REPO_ROOT
    / "crates"
    / "opendqi-cli"
    / "tests"
    / "golden"
    / "sftr-data-quality-pack.indicators.csv"
)
EVIDENCE_GOLDEN = (
    REPO_ROOT
    / "crates"
    / "opendqi-cli"
    / "tests"
    / "golden"
    / "sftr-data-quality-pack.evidence.csv"
)


def _all_fixtures_present() -> bool:
    return TSR_FIXTURE.exists() and TAR_FIXTURE.exists()


def test_at_least_one_input_required() -> None:
    """An empty SFTR pack (no inputs) is a user error, not a silent
    empty result — the function refuses with a clear message."""
    import opendqi

    with pytest.raises(ValueError, match="at least one"):
        opendqi.sftr.data_quality_pack(as_of="2026-05-21")


def test_pack_returns_dqi_pack_result() -> None:
    if not _all_fixtures_present():
        pytest.skip("SFTR fixtures missing")
    import opendqi

    result = opendqi.sftr.data_quality_pack(
        tsr=str(TSR_FIXTURE),
        tar=str(TAR_FIXTURE),
        as_of="2026-05-21",
    )
    assert result.indicators is not None
    assert result.evidence is not None
    assert result.issues is not None
    assert result.summary is not None
    r = repr(result)
    assert "PyDqiPackResult" in r
    assert "score=" in r


def test_indicators_table_is_4_rows() -> None:
    """v0.16 ships 4 SFTR indicators (T2 layer of auth.079 + auth.052).
    The T3-layer indicators + reconciliation/missing-collateral DQIs
    are scheduled for v0.17."""
    if not _all_fixtures_present():
        pytest.skip("SFTR fixtures missing")
    import opendqi

    result = opendqi.sftr.data_quality_pack(
        tsr=str(TSR_FIXTURE),
        tar=str(TAR_FIXTURE),
        as_of="2026-05-21",
    )
    assert result.indicators.num_rows == 4


def test_indicators_alphabetical_by_indicator_id() -> None:
    """Stable ordering is part of the public contract."""
    if not _all_fixtures_present():
        pytest.skip("SFTR fixtures missing")
    import opendqi

    result = opendqi.sftr.data_quality_pack(
        tsr=str(TSR_FIXTURE),
        tar=str(TAR_FIXTURE),
        as_of="2026-05-21",
    )
    ids = result.indicators.column("indicator_id").to_pylist()
    assert ids == sorted(ids)
    assert ids == [
        "DQI_COLLATERAL_VALUE_MISSING_SFTR",
        "DQI_LOAN_VALUE_MISSING_SFTR",
        "DQI_LOAN_VALUE_STALE_SFTR",
        "DQI_TIM_REPORTING_LATE_SFTR",
    ]


def test_tsr_only_pack_marks_tar_indicator_not_applicable() -> None:
    """Omitting the TAR layer must make DQI_TIM_REPORTING_LATE_SFTR
    self-report `not_applicable` — the symmetric semantic of the EMIR
    pack's NotApplicable indicators when their source layer is
    missing."""
    if not _all_fixtures_present():
        pytest.skip("SFTR fixtures missing")
    import opendqi

    result = opendqi.sftr.data_quality_pack(
        tsr=str(TSR_FIXTURE),
        as_of="2026-05-21",
    )
    statuses = dict(
        zip(
            result.indicators.column("indicator_id").to_pylist(),
            result.indicators.column("status").to_pylist(),
        )
    )
    assert statuses["DQI_TIM_REPORTING_LATE_SFTR"] == "not_applicable"
    # TSR-fed indicators should compute (status != not_applicable).
    for tsr_ind in (
        "DQI_LOAN_VALUE_MISSING_SFTR",
        "DQI_LOAN_VALUE_STALE_SFTR",
        "DQI_COLLATERAL_VALUE_MISSING_SFTR",
    ):
        assert statuses[tsr_ind] != "not_applicable", tsr_ind


def test_indicators_arrow_schema_locked() -> None:
    """The 11-column v1.0 indicators schema is frozen — these are the
    exact column names + arrow types downstream consumers depend on.
    Parity test against the CLI golden (column-for-column)."""
    if not _all_fixtures_present():
        pytest.skip("SFTR fixtures missing")
    if not INDICATORS_GOLDEN.exists():
        pytest.skip("CLI golden not generated yet")
    import opendqi
    import pyarrow as pa

    result = opendqi.sftr.data_quality_pack(
        tsr=str(TSR_FIXTURE),
        tar=str(TAR_FIXTURE),
        as_of="2026-05-21",
    )
    schema = result.indicators.schema
    cols = [f.name for f in schema]
    assert cols == [
        "indicator_id",
        "regime",
        "dimension",
        "table_scope",
        "numerator",
        "denominator",
        "rate",
        "threshold_amber",
        "threshold_red",
        "status",
        "description",
    ]
    # Type spot-checks on a few critical columns.
    assert schema.field("numerator").type == pa.uint64()
    assert schema.field("denominator").type == pa.uint64()
    assert schema.field("rate").type == pa.float64()
    assert schema.field("threshold_amber").type == pa.float64()
    assert schema.field("threshold_red").type == pa.float64()


def test_evidence_arrow_schema_locked() -> None:
    """The 7-column v1.0 evidence schema is frozen."""
    if not _all_fixtures_present():
        pytest.skip("SFTR fixtures missing")
    if not EVIDENCE_GOLDEN.exists():
        pytest.skip("CLI golden not generated yet")
    import opendqi

    result = opendqi.sftr.data_quality_pack(
        tsr=str(TSR_FIXTURE),
        tar=str(TAR_FIXTURE),
        as_of="2026-05-21",
    )
    cols = [f.name for f in result.evidence.schema]
    assert cols == [
        "indicator_id",
        "uti",
        "counterparty",
        "asset_class",
        "source_file",
        "observed_value",
        "explanation",
    ]


def test_parity_with_cli_golden_indicators() -> None:
    """The Python `result.indicators` must match the CLI golden
    indicators.csv byte-for-byte after CSV normalization (modulo
    ordering — both already sort alphabetically by indicator_id)."""
    if not _all_fixtures_present():
        pytest.skip("SFTR fixtures missing")
    if not INDICATORS_GOLDEN.exists():
        pytest.skip("CLI golden not generated yet")
    import csv

    import opendqi

    result = opendqi.sftr.data_quality_pack(
        tsr=str(TSR_FIXTURE),
        tar=str(TAR_FIXTURE),
        as_of="2026-05-21",
    )
    # Build dict-of-rows from the Python result.
    py_rows = {
        row["indicator_id"]: row for row in result.indicators.to_pylist()
    }
    # Read CLI golden into the same shape.
    with INDICATORS_GOLDEN.open(newline="") as f:
        reader = csv.DictReader(f)
        cli_rows = {row["indicator_id"]: row for row in reader}
    assert set(py_rows) == set(cli_rows)
    for ind_id, py_row in py_rows.items():
        cli_row = cli_rows[ind_id]
        # Numerator + denominator must match exactly (cast types).
        assert int(py_row["numerator"]) == int(cli_row["numerator"]), ind_id
        assert int(py_row["denominator"]) == int(cli_row["denominator"]), ind_id
        assert py_row["status"] == cli_row["status"], ind_id
        assert py_row["dimension"] == cli_row["dimension"], ind_id
        assert py_row["table_scope"] == cli_row["table_scope"], ind_id
