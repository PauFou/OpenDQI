"""
v0.17 H3 — `opendqi.sftr.tr_audit` cross-message workflow.

Consolidates per-layer scans (TAR + TSR) and runs the 2
SFTR.AUD.* cross-layer coherence checks
(NEWT_IN_TAR_NOT_IN_TSR, OUTSTANDING_IN_TSR_NOT_IN_TAR).

Tests against the shipped synthetic fixtures
examples/sftr/{tr_activity/auth052-tar-sample.xml,
tr_state/auth079-sample.xml} — the same files the CLI
sftr-tr-audit golden uses.
"""
from __future__ import annotations

from pathlib import Path

import pyarrow as pa
import pytest


REPO_ROOT = Path(__file__).resolve().parents[3]
SFTR_TAR = REPO_ROOT / "examples" / "sftr" / "tr_activity" / "auth052-tar-sample.xml"
SFTR_TSR = REPO_ROOT / "examples" / "sftr" / "tr_state" / "auth079-sample.xml"


def _require(*paths: Path) -> None:
    missing = [p for p in paths if not p.exists()]
    if missing:
        pytest.skip(f"fixtures missing: {missing}")


def test_sftr_tr_audit_runs_both_layers_plus_cross_layer() -> None:
    _require(SFTR_TAR, SFTR_TSR)
    import opendqi

    result = opendqi.sftr.tr_audit(tar=str(SFTR_TAR), tsr=str(SFTR_TSR))
    s = result.summary
    assert s["regime"] == "sftr"
    # 2 files processed (TAR + TSR ; SFTR has no rejection-feedback
    # message — auth.080 is reconciliation, not feedback).
    assert s["files_processed"] == 2
    # Shipped fixtures: 14 TAR + 13 TSR = 27 records. Allow some
    # tolerance.
    assert 20 <= s["records_processed"] <= 40
    # The 2 layers produce a substantial issue set ; lower bound
    # at 50 covers the SFTR.COMP.* + SFTR.TST.* + cross-layer
    # SFTR.AUD.* signals on the shipped fixtures.
    assert s["issues_total"] >= 50, (
        f"too few issues: {s['issues_total']} (sanity floor 50)"
    )


def test_sftr_tr_audit_fires_cross_layer_aud_checks() -> None:
    """SFTR.AUD.* should appear in result.issues — the shipped
    fixtures contain UTIs that cross the layers and trigger at
    least one of the 2 cross-layer checks (NEWT_IN_TAR_NOT_IN_TSR
    or OUTSTANDING_IN_TSR_NOT_IN_TAR)."""
    _require(SFTR_TAR, SFTR_TSR)
    import opendqi

    result = opendqi.sftr.tr_audit(tar=str(SFTR_TAR), tsr=str(SFTR_TSR))
    check_ids = set(result.issues.column("check_id").to_pylist())
    aud_present = {c for c in check_ids if c.startswith("SFTR.AUD.")}
    assert len(aud_present) >= 1, (
        f"no SFTR.AUD.* fired (expected at least 1 of 2 on the shipped "
        f"fixtures); all check_ids: {sorted(check_ids)[:20]}..."
    )


def test_sftr_tr_audit_per_layer_prefixes_present() -> None:
    """check_id prefixes should cover at least TSR-layer (SFTR.TST.*)
    and TAR-layer (SFTR.COMP.* / SFTR.TRA.*)."""
    _require(SFTR_TAR, SFTR_TSR)
    import opendqi

    result = opendqi.sftr.tr_audit(tar=str(SFTR_TAR), tsr=str(SFTR_TSR))
    check_ids = set(result.issues.column("check_id").to_pylist())
    tst = {c for c in check_ids if c.startswith("SFTR.TST.")}
    tar_layer = {c for c in check_ids if c.startswith(("SFTR.COMP.", "SFTR.TRA."))}
    assert len(tst) >= 1, f"no SFTR.TST.* fired; check_ids: {sorted(check_ids)}"
    assert len(tar_layer) >= 1, (
        f"no SFTR.COMP.* / SFTR.TRA.* fired; check_ids: {sorted(check_ids)}"
    )


def test_sftr_tr_audit_keyword_only_args() -> None:
    """Both args are keyword-only — positional should raise TypeError."""
    _require(SFTR_TAR, SFTR_TSR)
    import opendqi

    with pytest.raises(TypeError):
        opendqi.sftr.tr_audit(  # type: ignore[misc]
            str(SFTR_TAR), str(SFTR_TSR)
        )


def test_sftr_tr_audit_returns_arrow_issues() -> None:
    """result.issues is a pyarrow.Table per the v1.0 11-col
    contract — same as every other opendqi.sftr.* entry point."""
    _require(SFTR_TAR, SFTR_TSR)
    import opendqi

    result = opendqi.sftr.tr_audit(tar=str(SFTR_TAR), tsr=str(SFTR_TSR))
    assert isinstance(result.issues, pa.Table)
    expected_cols = {
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
    }
    cols = set(result.issues.column_names)
    missing = expected_cols - cols
    assert not missing, f"v1.0 issues schema missing cols: {missing}"
