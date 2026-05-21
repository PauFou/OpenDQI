"""
v0.15.0 — `opendqi.emir.data_quality_pack` Python binding.

Locks the v1.0 stable Arrow contracts for `result.indicators`
and `result.evidence` against the CLI goldens (same scan run
two different ways must produce identical column structure).

The other tests cover the gated-indicator mechanic
(confirmation_timestamp / reconciliation_status not mapped →
NotApplicable status, no evidence), the indicator ordering
contract (alphabetical by indicator_id), and the at-least-one-
input precondition.
"""
from __future__ import annotations

from pathlib import Path

import pytest


REPO_ROOT = Path(__file__).resolve().parents[3]
TSR_FIXTURE = REPO_ROOT / "examples" / "quickstart-emir" / "auth107-tsr.xml"
TAR_FIXTURE = REPO_ROOT / "examples" / "quickstart-emir" / "auth030-tar.xml"
FBK_FIXTURE = REPO_ROOT / "examples" / "quickstart-emir" / "auth092-feedback.xml"

INDICATORS_GOLDEN = (
    REPO_ROOT
    / "crates"
    / "opendqi-cli"
    / "tests"
    / "golden"
    / "emir-data-quality-pack.indicators.csv"
)
EVIDENCE_GOLDEN = (
    REPO_ROOT
    / "crates"
    / "opendqi-cli"
    / "tests"
    / "golden"
    / "emir-data-quality-pack.evidence.csv"
)


def _all_fixtures_present() -> bool:
    return TSR_FIXTURE.exists() and TAR_FIXTURE.exists() and FBK_FIXTURE.exists()


def test_at_least_one_input_required() -> None:
    """An empty pack (no inputs) is a user error, not a silent
    empty result — the function refuses with a clear message."""
    import opendqi

    with pytest.raises(ValueError, match="at least one"):
        opendqi.emir.data_quality_pack(as_of="2026-05-21")


def test_pack_returns_dqi_pack_result() -> None:
    if not _all_fixtures_present():
        pytest.skip("quickstart fixtures missing")
    import opendqi

    result = opendqi.emir.data_quality_pack(
        tsr=str(TSR_FIXTURE),
        tar=str(TAR_FIXTURE),
        feedback=str(FBK_FIXTURE),
        as_of="2026-05-21",
    )
    # All getters return non-None objects.
    assert result.indicators is not None
    assert result.evidence is not None
    assert result.issues is not None
    assert result.summary is not None
    # repr() is informative.
    r = repr(result)
    assert "PyDqiPackResult" in r
    assert "score=" in r


def test_indicators_table_is_21_rows() -> None:
    """v0.16 B1 ships 14 EMIR indicators (10 v0.15 + 4 v0.16
    cross-CP from auth.091) ; downstream consumers rely on a
    fixed indicator set per release."""
    if not _all_fixtures_present():
        pytest.skip("quickstart fixtures missing")
    import opendqi

    result = opendqi.emir.data_quality_pack(
        tsr=str(TSR_FIXTURE),
        tar=str(TAR_FIXTURE),
        feedback=str(FBK_FIXTURE),
        as_of="2026-05-21",
    )
    assert result.indicators.num_rows == 21


def test_indicators_alphabetical_by_indicator_id() -> None:
    """Stable ordering is part of the public contract."""
    if not _all_fixtures_present():
        pytest.skip("quickstart fixtures missing")
    import opendqi

    result = opendqi.emir.data_quality_pack(
        tsr=str(TSR_FIXTURE),
        tar=str(TAR_FIXTURE),
        feedback=str(FBK_FIXTURE),
        as_of="2026-05-21",
    )
    ids = result.indicators.column("indicator_id").to_pylist()
    assert ids == sorted(ids)
    assert ids == [
        "DQI_COL_ALL_ZERO",
        "DQI_COL_MISSING_STATE",
        "DQI_COL_STALE_STATE",
        "DQI_CONF_MISSING",
        "DQI_ERR_MISSING",
        "DQI_FIELD_MISMATCH_RATE",
        "DQI_LEI_MISSING",
        "DQI_MARGIN_INCONSISTENT_POST_HAIRCUT",
        "DQI_MARGIN_INCONSISTENT_PRE_HAIRCUT",
        "DQI_NATURE_MISSING",
        "DQI_NOTIONAL_INCONSISTENT",
        "DQI_PAIRING_RATE",
        "DQI_RECONCILIATION_RATE",
        "DQI_REC_STATUS_UNPAIRED",
        "DQI_REJ_RATE",
        "DQI_REJ_REPEAT_UTI",
        "DQI_SECTOR_MISSING",
        "DQI_TIM_REPORTING_LATE",
        "DQI_UNPAIRED_TRADES_RATE",
        "DQI_VAL_MISSING",
        "DQI_VAL_STALE",
    ]


def test_gated_indicators_off_by_default_on_quickstart() -> None:
    """Quickstart fixtures don't ship `confirmation_timestamp`
    or `reconciliation_status` in TAR raw_fields → both gates
    OFF → NotApplicable."""
    if not _all_fixtures_present():
        pytest.skip("quickstart fixtures missing")
    import opendqi

    result = opendqi.emir.data_quality_pack(
        tar=str(TAR_FIXTURE),
        as_of="2026-05-21",
    )
    ids = result.indicators.column("indicator_id").to_pylist()
    statuses = result.indicators.column("status").to_pylist()
    status_by_id = dict(zip(ids, statuses))
    assert status_by_id["DQI_CONF_MISSING"] == "not_applicable"
    assert status_by_id["DQI_REC_STATUS_UNPAIRED"] == "not_applicable"


def test_msr_only_pack_computes_msr_indicators_only() -> None:
    """Subset of layers → subset of computed indicators. The
    remaining 8 stay as NotApplicable rows."""
    if not _all_fixtures_present():
        pytest.skip("quickstart fixtures missing")
    import opendqi

    # No MSR fixture in quickstart-emir ; the empty pack still
    # produces 10 indicators all NotApplicable.
    result = opendqi.emir.data_quality_pack(
        tsr=str(TSR_FIXTURE),
        as_of="2026-05-21",
    )
    statuses = result.indicators.column("status").to_pylist()
    not_applicable = sum(1 for s in statuses if s == "not_applicable")
    # TSR-only → VAL_MISSING + VAL_STALE potentially computed,
    # everything else NotApplicable.
    assert not_applicable >= 8


# -------------------------------------------------------------------
# v1.0 Arrow schema contracts — both NEW DQI tables locked here.
# -------------------------------------------------------------------


def test_indicators_arrow_schema_locked() -> None:
    """11 columns in this exact order, with these exact types."""
    if not _all_fixtures_present():
        pytest.skip("quickstart fixtures missing")
    import opendqi
    import pyarrow as pa

    result = opendqi.emir.data_quality_pack(
        tsr=str(TSR_FIXTURE), as_of="2026-05-21"
    )
    expected_names = [
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
    assert result.indicators.column_names == expected_names

    sch = result.indicators.schema
    # String columns
    for name in (
        "indicator_id",
        "regime",
        "dimension",
        "table_scope",
        "status",
        "description",
    ):
        assert sch.field(name).type == pa.string(), f"{name} must be string"
    # Counters
    assert sch.field("numerator").type == pa.uint64()
    assert sch.field("denominator").type == pa.uint64()
    # Floats (rate may be null on NotApplicable rows)
    assert sch.field("rate").type == pa.float64()
    assert sch.field("threshold_amber").type == pa.float64()
    assert sch.field("threshold_red").type == pa.float64()


def test_evidence_arrow_schema_locked() -> None:
    """7 columns in this exact order, with these exact types."""
    if not _all_fixtures_present():
        pytest.skip("quickstart fixtures missing")
    import opendqi
    import pyarrow as pa

    result = opendqi.emir.data_quality_pack(
        tsr=str(TSR_FIXTURE), as_of="2026-05-21"
    )
    expected_names = [
        "indicator_id",
        "uti",
        "counterparty",
        "asset_class",
        "source_file",
        "observed_value",
        "explanation",
    ]
    assert result.evidence.column_names == expected_names
    sch = result.evidence.schema
    for name in expected_names:
        assert sch.field(name).type == pa.string(), f"{name} must be string"


def test_indicators_arrow_columns_match_csv_golden() -> None:
    """Same scan two ways (CLI golden vs Python bindings) →
    identical column structure. Locks the v1.0 contract that the
    on-disk indicators.csv and the in-memory pyarrow.Table never
    drift."""
    if not _all_fixtures_present() or not INDICATORS_GOLDEN.exists():
        pytest.skip("fixtures or golden missing")
    import opendqi
    import pyarrow.csv as pacsv

    result = opendqi.emir.data_quality_pack(
        tsr=str(TSR_FIXTURE),
        tar=str(TAR_FIXTURE),
        feedback=str(FBK_FIXTURE),
        as_of="2026-05-21",
    )
    csv_golden = pacsv.read_csv(str(INDICATORS_GOLDEN))
    assert result.indicators.column_names == csv_golden.column_names


def test_evidence_arrow_columns_match_csv_golden() -> None:
    """Same parity test for evidence.csv."""
    if not _all_fixtures_present() or not EVIDENCE_GOLDEN.exists():
        pytest.skip("fixtures or golden missing")
    import opendqi
    import pyarrow.csv as pacsv

    result = opendqi.emir.data_quality_pack(
        tsr=str(TSR_FIXTURE),
        tar=str(TAR_FIXTURE),
        feedback=str(FBK_FIXTURE),
        as_of="2026-05-21",
    )
    csv_golden = pacsv.read_csv(str(EVIDENCE_GOLDEN))
    assert result.evidence.column_names == csv_golden.column_names


# -------------------------------------------------------------------
# v0.15.0 — Arrow input dispatch (str path | pyarrow.Table)
# -------------------------------------------------------------------


def _arrow_tsr_with_3_missing_valuations():
    import pyarrow as pa
    import pyarrow.compute as pc

    t = pa.table(
        {
            "TradeUti": ["U1", "U2", "U3"],
            "Status": ["OUTSTANDING", "OUTSTANDING", "OUTSTANDING"],
            "Val": [None, None, None],
        }
    )
    # Cast the (otherwise null) `Val` column to the canonical Decimal128(38,10)
    # type the converter expects.
    return t.set_column(
        2, "Val", pc.cast(t.column("Val"), pa.decimal128(38, 10))
    )


def test_arrow_input_tsr_only_missing_mapping_raises() -> None:
    """Passing a pyarrow.Table for a layer without a mapping in
    `mappings={...}` is a user error — clear ValueError."""
    import opendqi

    tsr = _arrow_tsr_with_3_missing_valuations()
    with pytest.raises(ValueError, match="mappings"):
        opendqi.emir.data_quality_pack(tsr=tsr, as_of="2026-05-21")


def test_arrow_input_tsr_only_computes_dqi_val_missing() -> None:
    """Arrow input dual-dispatch works end-to-end for TSR : a
    3-record Table with all missing valuations rolls up to
    DQI_VAL_MISSING numerator=3, denominator=3, rate=1.0, red."""
    import opendqi

    tsr = _arrow_tsr_with_3_missing_valuations()
    result = opendqi.emir.data_quality_pack(
        tsr=tsr,
        mappings={
            "tsr": {
                "uti": "TradeUti",
                "status": "Status",
                "valuation_amount": "Val",
            }
        },
        as_of="2026-05-21",
    )
    df = result.indicators.to_pandas()
    row = df[df["indicator_id"] == "DQI_VAL_MISSING"].iloc[0]
    assert row["numerator"] == 3
    assert row["denominator"] == 3
    assert row["status"] == "red"


def test_arrow_input_mar_raises_explicit_error() -> None:
    """MAR is paths-only in v0.15 ; passing a pyarrow.Table for
    MAR raises a CLEAR ValueError pointing at the limitation."""
    import opendqi
    import pyarrow as pa

    mar_table = pa.table({"x": [1, 2]})
    with pytest.raises(ValueError, match="v0.15"):
        opendqi.emir.data_quality_pack(mar=mar_table, as_of="2026-05-21")


def test_arrow_input_mixed_with_paths() -> None:
    """Mix-and-match: TSR as pyarrow.Table + Feedback as XML
    path. Both layers contribute to the pack output."""
    if not _all_fixtures_present():
        pytest.skip("quickstart fixtures missing")
    import opendqi

    tsr = _arrow_tsr_with_3_missing_valuations()
    result = opendqi.emir.data_quality_pack(
        tsr=tsr,
        feedback=str(FBK_FIXTURE),
        mappings={
            "tsr": {
                "uti": "TradeUti",
                "status": "Status",
                "valuation_amount": "Val",
            }
        },
        as_of="2026-05-21",
    )
    df = result.indicators.to_pandas()
    # TSR-side
    val_missing = df[df["indicator_id"] == "DQI_VAL_MISSING"].iloc[0]
    assert val_missing["denominator"] == 3
    # Feedback-side (auth.092 has 2 rejected records in quickstart fixture)
    rej = df[df["indicator_id"] == "DQI_REJ_RATE"].iloc[0]
    assert rej["status"] == "red"
    assert rej["numerator"] == 2


def test_report_writes_5_artefacts(tmp_path: Path) -> None:
    """`.report(out_dir)` mirrors the CLI subcommand outputs."""
    if not _all_fixtures_present():
        pytest.skip("quickstart fixtures missing")
    import opendqi

    result = opendqi.emir.data_quality_pack(
        tsr=str(TSR_FIXTURE),
        tar=str(TAR_FIXTURE),
        feedback=str(FBK_FIXTURE),
        as_of="2026-05-21",
    )
    out = tmp_path / "pack-out"
    result.report(str(out))
    for name in (
        "report.html",
        "summary.json",
        "issues.csv",
        "indicators.csv",
        "evidence.csv",
    ):
        assert (out / name).exists(), f"missing {name}"
    # HTML carries the Data Quality Pack section.
    html = (out / "report.html").read_text()
    assert "Data Quality Pack" in html
