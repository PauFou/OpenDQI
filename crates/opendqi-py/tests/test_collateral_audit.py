"""
v0.13.0 — `opendqi.emir.collateral_audit` + `opendqi.sftr.missing_collateral`.

EMIR collateral_audit: TSR(auth.107) ⊗ MSR(auth.109) join, fires
EMIR.COL.MISSING and/or EMIR.COL.STALE per Article 11.

SFTR missing_collateral: parses auth.083 (Missing Collateral
Request), runs 2 base SFTR.MCR.* checks; optional cross-ref
against auth.079 TSR fires 3 additional MCR cross-checks.
"""
from __future__ import annotations

from pathlib import Path

import pytest


REPO_ROOT = Path(__file__).resolve().parents[3]
EMIR_TSR_COL = REPO_ROOT / "examples" / "emir" / "collateral_audit" / "tsr.xml"
EMIR_MSR_COL = REPO_ROOT / "examples" / "emir" / "collateral_audit" / "msr.xml"
SFTR_AUTH083 = REPO_ROOT / "examples" / "sftr" / "missing_collateral" / "auth083-sample.xml"
SFTR_TSR = REPO_ROOT / "examples" / "sftr" / "tr_state" / "auth079-sample.xml"


def _require(*paths: Path) -> None:
    missing = [p for p in paths if not p.exists()]
    if missing:
        pytest.skip(f"fixtures missing: {missing}")


# --- EMIR collateral_audit ---------------------------------------


def test_emir_collateral_audit_basic() -> None:
    _require(EMIR_TSR_COL, EMIR_MSR_COL)
    import opendqi
    result = opendqi.emir.collateral_audit(
        tsr=str(EMIR_TSR_COL), msr=str(EMIR_MSR_COL)
    )
    s = result.summary
    assert s["regime"] == "emir"
    assert s["files_processed"] == 2
    # On the shipped fixture (4 TSR derivatives × ~3 MSR rows) we
    # expect a small handful of EMIR.COL.* issues. Don't assert an
    # exact number since the fixture may evolve; assert > 0.
    assert s["issues_total"] > 0


def test_emir_collateral_audit_fires_col_checks_only() -> None:
    _require(EMIR_TSR_COL, EMIR_MSR_COL)
    import opendqi
    result = opendqi.emir.collateral_audit(
        tsr=str(EMIR_TSR_COL), msr=str(EMIR_MSR_COL)
    )
    check_ids = set(result.issues.column("check_id").to_pylist())
    # Every issue should be EMIR.COL.* — this entry point ONLY
    # runs the cross-message collateral checks, nothing else.
    non_col = {c for c in check_ids if not c.startswith("EMIR.COL.")}
    assert not non_col, f"non-COL checks slipped in: {non_col}"
    # At least one of MISSING or STALE should fire.
    assert any(c in check_ids for c in (
        "EMIR.COL.MISSING", "EMIR.COL.STALE"
    )), f"neither EMIR.COL.MISSING nor STALE fired; got: {check_ids}"


def test_emir_collateral_audit_keyword_only() -> None:
    _require(EMIR_TSR_COL, EMIR_MSR_COL)
    import opendqi
    with pytest.raises(TypeError):
        opendqi.emir.collateral_audit(  # type: ignore[misc]
            str(EMIR_TSR_COL), str(EMIR_MSR_COL)
        )


# --- SFTR missing_collateral -------------------------------------


def test_sftr_missing_collateral_base_only() -> None:
    """Without --tsr, only the 2 base SFTR.MCR.* checks run."""
    _require(SFTR_AUTH083)
    import opendqi
    result = opendqi.sftr.missing_collateral(str(SFTR_AUTH083))
    s = result.summary
    assert s["regime"] == "sftr"
    assert s["files_processed"] == 1
    # MCR base fires at least once on the shipped fixture.
    assert s["issues_total"] >= 1


def test_sftr_missing_collateral_with_tsr_cross_ref() -> None:
    """With --tsr, the 3 cross-ref checks also run; total >= base-only."""
    _require(SFTR_AUTH083, SFTR_TSR)
    import opendqi
    r_base = opendqi.sftr.missing_collateral(str(SFTR_AUTH083))
    r_xref = opendqi.sftr.missing_collateral(str(SFTR_AUTH083), tsr=str(SFTR_TSR))
    # Cross-ref version processes 2 files
    assert r_xref.summary["files_processed"] == 2
    # And has >= as many issues (cross-ref adds — never removes).
    assert r_xref.summary["issues_total"] >= r_base.summary["issues_total"]


def test_sftr_missing_collateral_function_present() -> None:
    import opendqi
    assert callable(opendqi.sftr.missing_collateral)
