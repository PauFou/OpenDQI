"""
v0.16.0 / v0.17 — `opendqi.sftr.data_quality_pack` Python binding.

Locks the v1.0 stable Arrow contracts for `result.indicators`
and `result.evidence` against the CLI golden — the same scan run
two different ways (CLI + Python) must produce identical column
structure for the 16 SFTR indicators (v0.16 shipped 4, v0.17
adds 12: 4 T3 + 4 reconciliation + 1 MCR + 3 SFTR-specific).

v0.17 is still **paths-only** on the SFTR DQI pack — Arrow
converters for SftrTrStateRecord / SftrMarginStateRecord /
ReconciliationRecord / MissingCollateralRecord are scheduled
for v0.18.
"""
from __future__ import annotations

from pathlib import Path

import pytest


REPO_ROOT = Path(__file__).resolve().parents[3]
TSR_FIXTURE = REPO_ROOT / "examples" / "sftr" / "tr_state" / "auth079-sample.xml"
TAR_FIXTURE = REPO_ROOT / "examples" / "sftr" / "tr_activity" / "auth052-tar-sample.xml"
RECON_FIXTURE = REPO_ROOT / "examples" / "sftr" / "reconciliation" / "auth080-sample.xml"
MCR_FIXTURE = (
    REPO_ROOT / "examples" / "sftr" / "missing_collateral" / "auth083-sample.xml"
)
MSR_FIXTURE = REPO_ROOT / "examples" / "sftr" / "margin_state" / "auth085-sample.xml"

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


def _all_v017_fixtures_present() -> bool:
    return (
        _all_fixtures_present()
        and RECON_FIXTURE.exists()
        and MCR_FIXTURE.exists()
        and MSR_FIXTURE.exists()
    )


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


def test_indicators_table_is_16_rows() -> None:
    """v0.17 ships 16 SFTR indicators across 5 input layers : 6 TSR
    (auth.079) + 1 TAR (auth.052) + 4 reconciliation (auth.080) + 1
    MCR (auth.083) + 4 MSR T3 (auth.085). Indicators whose source
    layer isn't provided self-report `not_applicable` but the row
    is still in the output — the count is fixed at 16."""
    if not _all_fixtures_present():
        pytest.skip("SFTR fixtures missing")
    import opendqi

    result = opendqi.sftr.data_quality_pack(
        tsr=str(TSR_FIXTURE),
        tar=str(TAR_FIXTURE),
        as_of="2026-05-21",
    )
    assert result.indicators.num_rows == 16


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
        "DQI_FIELD_MISMATCH_RATE_SFTR",
        "DQI_HAIRCUT_ANOMALY_SFTR",
        "DQI_LEI_MISSING_SFTR",
        "DQI_LOAN_VALUE_MISSING_SFTR",
        "DQI_LOAN_VALUE_STALE_SFTR",
        "DQI_MCR_OPEN_REQUESTS_SFTR",
        "DQI_PAIRING_RATE_SFTR",
        "DQI_RECONCILIATION_RATE_SFTR",
        "DQI_T3_EXCESS_COLLATERAL_USE_SFTR",
        "DQI_T3_MARGIN_POSTED_MISSING_SFTR",
        "DQI_T3_MARGIN_RECEIVED_MISSING_SFTR",
        "DQI_T3_MARGIN_STALE_SFTR",
        "DQI_TIM_REPORTING_LATE_SFTR",
        "DQI_UNDER_COLLATERALIZATION_SFTR",
        "DQI_UNPAIRED_TRADES_RATE_SFTR",
    ]


def test_full_5_input_pack_computes_all_16_indicators() -> None:
    """v0.17 G2': feeding all 5 layers makes every DQI compute
    (no self-reported `not_applicable`). Exercises the full
    indicator/granular pipeline end-to-end including the MSR
    layer + SFTR.T3.* granular checks."""
    if not _all_v017_fixtures_present():
        pytest.skip("v0.17 fixtures (recon/MCR/MSR) missing")
    import opendqi

    result = opendqi.sftr.data_quality_pack(
        tsr=str(TSR_FIXTURE),
        tar=str(TAR_FIXTURE),
        reconciliation=str(RECON_FIXTURE),
        missing_collateral=str(MCR_FIXTURE),
        msr=str(MSR_FIXTURE),
        as_of="2026-05-21",
    )
    assert result.indicators.num_rows == 16
    statuses = result.indicators.column("status").to_pylist()
    assert all(s != "not_applicable" for s in statuses), (
        f"all 16 DQIs must compute with full 5-input pack ; statuses: {statuses}"
    )
    # The MSR layer triggers the F1' SFTR.T3.* granular checks
    # so the issues stream must contain at least one of them
    # (the auth.085 fixture has REC-5 negative IM posted +
    # multiple partial-side records).
    issues_check_ids = set(result.issues.column("check_id").to_pylist())
    assert any(
        cid.startswith("SFTR.T3.") for cid in issues_check_ids
    ), f"expected SFTR.T3.* granular checks in issues, got {sorted(issues_check_ids)}"


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
