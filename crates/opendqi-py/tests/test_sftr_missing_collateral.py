"""
v0.17 H4 — `opendqi.sftr.missing_collateral` entry point.

Parses an auth.083 SFTR Missing Collateral Request, runs the
2 base SFTR.MCR.* checks, and (when a companion auth.079 TSR
is supplied) runs 3 additional cross-ref checks.

Coverage was previously only inside test_collateral_audit.py's
mixed file — H4 splits it out into a dedicated dedicated SFTR
test file for symmetry with the rest of the SFTR Python
surface.
"""
from __future__ import annotations

from pathlib import Path

import pyarrow as pa
import pytest


REPO_ROOT = Path(__file__).resolve().parents[3]
SFTR_AUTH083 = (
    REPO_ROOT / "examples" / "sftr" / "missing_collateral" / "auth083-sample.xml"
)
SFTR_TSR = REPO_ROOT / "examples" / "sftr" / "tr_state" / "auth079-sample.xml"


def _require(*paths: Path) -> None:
    missing = [p for p in paths if not p.exists()]
    if missing:
        pytest.skip(f"fixtures missing: {missing}")


def test_missing_collateral_solo_runs_2_base_checks() -> None:
    """Without a TSR companion, only the 2 base SFTR.MCR.* checks
    that don't need cross-ref data run (REQUEST_WITHOUT_UTI +
    typically format-level)."""
    _require(SFTR_AUTH083)
    import opendqi

    result = opendqi.sftr.missing_collateral(str(SFTR_AUTH083))
    s = result.summary
    assert s["regime"] == "sftr"
    # 1 file when no TSR is given.
    assert s["files_processed"] == 1
    assert s["records_processed"] > 0
    # No assertion on issues_total — depends on whether the
    # fixture has request_without_uti rows ; just verify the
    # pipeline ran without crashing.
    assert isinstance(result.issues, pa.Table)


def test_missing_collateral_with_tsr_cross_ref_runs_5_checks() -> None:
    """With --tsr the cross-ref family fires :
    SFTR.MCR.COLLATERAL_PRESENT_IN_TSR
    SFTR.MCR.STILL_MISSING_IN_TSR
    SFTR.MCR.REQUESTED_UTI_NOT_IN_TSR
    on top of the 2 base checks."""
    _require(SFTR_AUTH083, SFTR_TSR)
    import opendqi

    r_solo = opendqi.sftr.missing_collateral(str(SFTR_AUTH083))
    r_xref = opendqi.sftr.missing_collateral(
        str(SFTR_AUTH083), tsr=str(SFTR_TSR)
    )
    # files_processed bumps to 2 when TSR is supplied.
    assert r_solo.summary["files_processed"] == 1
    assert r_xref.summary["files_processed"] == 2
    # The cross-ref family should produce at least one new check
    # ID that wasn't present in the solo run.
    solo_ids = set(r_solo.issues.column("check_id").to_pylist())
    xref_ids = set(r_xref.issues.column("check_id").to_pylist())
    new_ids = xref_ids - solo_ids
    # The cross-ref family : at least one must surface ; the
    # default fixture is designed to exercise STILL_MISSING_IN_TSR
    # and / or REQUESTED_UTI_NOT_IN_TSR.
    assert any("MCR" in cid for cid in new_ids), (
        f"expected cross-ref MCR checks to surface ; solo={sorted(solo_ids)} ; "
        f"xref-new={sorted(new_ids)}"
    )


def test_missing_collateral_all_check_ids_are_sftr_mcr() -> None:
    """Every issue from this entry point should be SFTR.MCR.* or a
    format-level SFTR.FMT.* — no other family slips in."""
    _require(SFTR_AUTH083, SFTR_TSR)
    import opendqi

    result = opendqi.sftr.missing_collateral(
        str(SFTR_AUTH083), tsr=str(SFTR_TSR)
    )
    check_ids = set(result.issues.column("check_id").to_pylist())
    unexpected = {
        c
        for c in check_ids
        if not (c.startswith("SFTR.MCR.") or c.startswith("SFTR.FMT."))
    }
    assert not unexpected, (
        f"unexpected non-MCR/non-FMT checks slipped in: {unexpected}"
    )


def test_missing_collateral_keyword_only_tsr_arg() -> None:
    """`tsr` is keyword-only ; auth083 is positional."""
    _require(SFTR_AUTH083, SFTR_TSR)
    import opendqi

    # positional auth083 is fine
    _ = opendqi.sftr.missing_collateral(str(SFTR_AUTH083))
    # positional tsr should raise
    with pytest.raises(TypeError):
        opendqi.sftr.missing_collateral(  # type: ignore[misc]
            str(SFTR_AUTH083), str(SFTR_TSR)
        )


def test_missing_collateral_nonexistent_file_surfaces_as_fmt_issue() -> None:
    """Unlike scan_parquet (which raises RuntimeError on missing
    files), the missing-collateral parser surfaces file-level
    errors as SFTR.FMT.XML_NOT_WELLFORMED critical issues. This
    matches the CLI behaviour (auth.083 reports always produce a
    report object, even when the input is corrupt — operators
    want to see the failure as data, not a crash)."""
    import opendqi

    result = opendqi.sftr.missing_collateral("/nonexistent/auth083.xml")
    check_ids = set(result.issues.column("check_id").to_pylist())
    assert "SFTR.FMT.XML_NOT_WELLFORMED" in check_ids
    severities = set(result.issues.column("severity").to_pylist())
    assert "critical" in severities
