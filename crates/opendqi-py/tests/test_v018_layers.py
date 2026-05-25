"""
v0.18 — Python binding coverage for the new layers and the
v0.16 EMIR auth.091 carry-over wiring (F1).

Scope of this file:
  - SFTR auth.070 MAR    (--mar / mar=)             — Phase A
  - SFTR auth.071 reuse  (--reuse-activity / reuse_activity=) — Phase B
  - SFTR auth.086 reuse-state (--reuse-state / reuse_state=) — Phase C
  - SFTR auth.084 TR status advice (--tr-status-advice / tr_status_advice=) — Phase D
  - EMIR auth.090 positions (--positions / positions=) — Phase E
  - EMIR auth.091 recon stats (--recon-stats / recon_stats=) — Phase F1

Pattern: smoke parse + DQI activation check per layer. The
parser correctness itself is covered by the Rust integration
tests in opendqi-xml; the Python tests confirm the binding
surface — kwargs accepted, results well-formed, indicator
shapes per the v0.18 contract.
"""
from __future__ import annotations

from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[3]

# SFTR fixtures (Phase A/B/C/D)
SFTR_MAR_FIXTURE = REPO_ROOT / "examples" / "sftr" / "margin_activity" / "auth070-sample.xml"
SFTR_REUSE_ACTIVITY_FIXTURE = (
    REPO_ROOT / "examples" / "sftr" / "reuse_activity" / "auth071-sample.xml"
)
SFTR_REUSE_STATE_FIXTURE = (
    REPO_ROOT / "examples" / "sftr" / "reuse_state" / "auth086-sample.xml"
)
SFTR_TSA_FIXTURE = (
    REPO_ROOT / "examples" / "sftr" / "tr_status_advice" / "auth084-sample.xml"
)

# EMIR fixtures (Phase E + F1)
EMIR_POSITIONS_FIXTURE = REPO_ROOT / "examples" / "emir" / "positions" / "auth090-sample.xml"
EMIR_RECON_STATS_FIXTURE = (
    REPO_ROOT / "examples" / "emir" / "recon_stats" / "auth091-sample.xml"
)


# ----- SFTR layers --------------------------------------------------------------


def test_sftr_mar_layer_activates_3_dqis() -> None:
    """Phase A: --mar / mar= activates the 3 SFTR MAR DQIs."""
    if not SFTR_MAR_FIXTURE.exists():
        pytest.skip("SFTR MAR fixture missing")
    import opendqi

    result = opendqi.sftr.data_quality_pack(
        mar=str(SFTR_MAR_FIXTURE), as_of="2026-05-12"
    )
    # 24 total SFTR DQIs (v0.18 count); the 3 MAR ones should compute,
    # the other 21 self-report not_applicable.
    assert result.indicators.num_rows == 24
    statuses = dict(
        zip(
            result.indicators.column("indicator_id").to_pylist(),
            result.indicators.column("status").to_pylist(),
        )
    )
    for mar_id in (
        "DQI_MAR_PARTIAL_SIDES_SFTR",
        "DQI_MAR_EXCESS_COLLATERAL_EVENT_RATE_SFTR",
        "DQI_MAR_EVENT_SPIKE_SFTR",
    ):
        assert statuses[mar_id] != "not_applicable", (
            f"{mar_id} should compute when --mar is provided; got {statuses[mar_id]}"
        )


def test_sftr_reuse_activity_layer_activates_2_dqis() -> None:
    """Phase B: --reuse-activity / reuse_activity= activates the 2 SFTR reuse DQIs."""
    if not SFTR_REUSE_ACTIVITY_FIXTURE.exists():
        pytest.skip("SFTR reuse activity fixture missing")
    import opendqi

    result = opendqi.sftr.data_quality_pack(
        reuse_activity=str(SFTR_REUSE_ACTIVITY_FIXTURE), as_of="2026-05-12"
    )
    assert result.indicators.num_rows == 24
    statuses = dict(
        zip(
            result.indicators.column("indicator_id").to_pylist(),
            result.indicators.column("status").to_pylist(),
        )
    )
    for reuse_id in (
        "DQI_REUSE_VOLUME_MISSING_SFTR",
        "DQI_REUSE_ERR_RETRACTION_RATE_SFTR",
    ):
        assert statuses[reuse_id] != "not_applicable", (
            f"{reuse_id} should compute when --reuse-activity is provided"
        )


def test_sftr_reuse_state_layer_activates_2_dqis() -> None:
    """Phase C: --reuse-state / reuse_state= activates the 2 SFTR reuse-state DQIs."""
    if not SFTR_REUSE_STATE_FIXTURE.exists():
        pytest.skip("SFTR reuse state fixture missing")
    import opendqi

    result = opendqi.sftr.data_quality_pack(
        reuse_state=str(SFTR_REUSE_STATE_FIXTURE), as_of="2026-05-12"
    )
    assert result.indicators.num_rows == 24
    statuses = dict(
        zip(
            result.indicators.column("indicator_id").to_pylist(),
            result.indicators.column("status").to_pylist(),
        )
    )
    for state_id in (
        "DQI_REUSE_STATE_VOLUME_MISSING_SFTR",
        "DQI_REUSE_STATE_STALE_SFTR",
    ):
        assert statuses[state_id] != "not_applicable", (
            f"{state_id} should compute when --reuse-state is provided"
        )


def test_sftr_tr_status_advice_layer_activates_rej_rate() -> None:
    """Phase D: --tr-status-advice / tr_status_advice= activates DQI_REJ_RATE_SFTR."""
    if not SFTR_TSA_FIXTURE.exists():
        pytest.skip("SFTR TR status advice fixture missing")
    import opendqi

    result = opendqi.sftr.data_quality_pack(
        tr_status_advice=str(SFTR_TSA_FIXTURE), as_of="2026-05-12"
    )
    statuses = dict(
        zip(
            result.indicators.column("indicator_id").to_pylist(),
            result.indicators.column("status").to_pylist(),
        )
    )
    assert statuses["DQI_REJ_RATE_SFTR"] != "not_applicable"
    # Fixture is 45 rejected / 1000 total = 4.5 % → green by default.
    assert statuses["DQI_REJ_RATE_SFTR"] == "green"


def test_sftr_all_4_v018_layers_combined() -> None:
    """End-to-end: all 4 v0.18 SFTR layers in one call. All 4
    new DQI families should compute simultaneously."""
    if not all(
        f.exists()
        for f in (
            SFTR_MAR_FIXTURE,
            SFTR_REUSE_ACTIVITY_FIXTURE,
            SFTR_REUSE_STATE_FIXTURE,
            SFTR_TSA_FIXTURE,
        )
    ):
        pytest.skip("v0.18 SFTR fixtures missing")
    import opendqi

    result = opendqi.sftr.data_quality_pack(
        mar=str(SFTR_MAR_FIXTURE),
        reuse_activity=str(SFTR_REUSE_ACTIVITY_FIXTURE),
        reuse_state=str(SFTR_REUSE_STATE_FIXTURE),
        tr_status_advice=str(SFTR_TSA_FIXTURE),
        as_of="2026-05-12",
    )
    statuses = dict(
        zip(
            result.indicators.column("indicator_id").to_pylist(),
            result.indicators.column("status").to_pylist(),
        )
    )
    # 8 v0.18 SFTR DQIs across the 4 layers: 3 MAR + 2 reuse +
    # 2 reuse-state + 1 REJ_RATE.
    v018_sftr_ids = [
        "DQI_MAR_PARTIAL_SIDES_SFTR",
        "DQI_MAR_EXCESS_COLLATERAL_EVENT_RATE_SFTR",
        "DQI_MAR_EVENT_SPIKE_SFTR",
        "DQI_REUSE_VOLUME_MISSING_SFTR",
        "DQI_REUSE_ERR_RETRACTION_RATE_SFTR",
        "DQI_REUSE_STATE_VOLUME_MISSING_SFTR",
        "DQI_REUSE_STATE_STALE_SFTR",
        "DQI_REJ_RATE_SFTR",
    ]
    for ind_id in v018_sftr_ids:
        assert statuses[ind_id] != "not_applicable", (
            f"{ind_id} must compute with the 4 v0.18 layers wired in"
        )


# ----- EMIR layers --------------------------------------------------------------


def test_emir_positions_layer_activates_4_dqis() -> None:
    """Phase E: --positions / positions= activates the 4 EMIR Position DQIs."""
    if not EMIR_POSITIONS_FIXTURE.exists():
        pytest.skip("EMIR positions fixture missing")
    import opendqi

    result = opendqi.emir.data_quality_pack(
        positions=str(EMIR_POSITIONS_FIXTURE), as_of="2026-05-21"
    )
    # 28 total EMIR DQIs (v0.18 count).
    assert result.indicators.num_rows == 28
    statuses = dict(
        zip(
            result.indicators.column("indicator_id").to_pylist(),
            result.indicators.column("status").to_pylist(),
        )
    )
    for pos_id in (
        "DQI_POSITION_NOTIONAL_MISSING",
        "DQI_POSITION_MARK_TO_MARKET_MISSING",
        "DQI_POSITION_NOTIONAL_NEGATIVE",
        "DQI_POSITION_COLLATERAL_MISSING",
    ):
        assert statuses[pos_id] != "not_applicable", (
            f"{pos_id} should compute when --positions is provided"
        )


def test_emir_recon_stats_activates_7_carry_over_dqis() -> None:
    """Phase F1: --recon-stats / recon_stats= activates the 7
    cross-CP DQIs that self-reported `not_applicable` since v0.16."""
    if not EMIR_RECON_STATS_FIXTURE.exists():
        pytest.skip("EMIR recon stats fixture missing")
    import opendqi

    result = opendqi.emir.data_quality_pack(
        recon_stats=str(EMIR_RECON_STATS_FIXTURE), as_of="2026-05-21"
    )
    statuses = dict(
        zip(
            result.indicators.column("indicator_id").to_pylist(),
            result.indicators.column("status").to_pylist(),
        )
    )
    v016_cross_cp_ids = [
        # v0.16 B1 — 4 from auth.091 cohort summary stats
        "DQI_PAIRING_RATE",
        "DQI_RECONCILIATION_RATE",
        "DQI_UNPAIRED_TRADES_RATE",
        "DQI_FIELD_MISMATCH_RATE",
        # v0.16 B4 — 3 from auth.091 per-tx detail
        "DQI_NOTIONAL_INCONSISTENT",
        "DQI_MARGIN_INCONSISTENT_PRE_HAIRCUT",
        "DQI_MARGIN_INCONSISTENT_POST_HAIRCUT",
    ]
    for ind_id in v016_cross_cp_ids:
        assert statuses[ind_id] != "not_applicable", (
            f"{ind_id} should compute when --recon-stats is provided; "
            f"got {statuses[ind_id]}"
        )
