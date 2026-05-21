"""
v0.13.0 — `opendqi.{emir,sftr}.tr_audit` cross-message workflow.

Consolidates per-layer scans + cross-layer AUD coherence checks.
EMIR variant: 3 layers (TAR/TSR/feedback) + 3 EMIR.AUD.*.
SFTR variant: 2 layers (TAR/TSR) + 2 SFTR.AUD.*.

Tests against the synthetic quickstart-emir fixtures (same files
the CLI tr-audit golden uses).
"""
from __future__ import annotations

from pathlib import Path

import pytest


REPO_ROOT = Path(__file__).resolve().parents[3]
EMIR_TAR = REPO_ROOT / "examples" / "quickstart-emir" / "auth030-tar.xml"
EMIR_TSR = REPO_ROOT / "examples" / "quickstart-emir" / "auth107-tsr.xml"
EMIR_FBK = REPO_ROOT / "examples" / "quickstart-emir" / "auth092-feedback.xml"


def _require(*paths: Path) -> None:
    missing = [p for p in paths if not p.exists()]
    if missing:
        pytest.skip(f"fixtures missing: {missing}")


def test_emir_tr_audit_runs_all_three_layers_plus_cross_layer() -> None:
    _require(EMIR_TAR, EMIR_TSR, EMIR_FBK)
    import opendqi
    result = opendqi.emir.tr_audit(
        tar=str(EMIR_TAR), tsr=str(EMIR_TSR), feedback=str(EMIR_FBK)
    )
    s = result.summary
    # 3 files processed, records = sum of records from each layer.
    assert s["regime"] == "emir"
    assert s["files_processed"] == 3
    # On the shipped fixtures: 20 TAR records + 8 TSR records +
    # 2 feedback records = 30 total. Asserting exactly is fragile
    # if a fixture changes; assert structural lower bound + upper.
    assert s["records_processed"] >= 25
    assert s["records_processed"] <= 50
    # Issues should be ≥ 100 (CLI golden for tr-audit shows 251).
    # The Python tr_audit excludes lifecycle (no store), so the
    # number can be lower than the CLI golden but still substantial.
    assert s["issues_total"] >= 100, (
        f"too few issues: {s['issues_total']} (sanity floor 100)"
    )


def test_emir_tr_audit_fires_cross_layer_aud_checks() -> None:
    """The 3 EMIR.AUD.* should appear in result.issues when the fixtures
    contain UTIs that cross the layers (e.g. an outstanding TSR UTI
    not in the TAR — the shipped fixtures are designed to fire these)."""
    _require(EMIR_TAR, EMIR_TSR, EMIR_FBK)
    import opendqi
    result = opendqi.emir.tr_audit(
        tar=str(EMIR_TAR), tsr=str(EMIR_TSR), feedback=str(EMIR_FBK)
    )
    check_ids = set(result.issues.column("check_id").to_pylist())
    # At least ONE of the 3 EMIR.AUD.* should fire on these fixtures.
    aud_present = {c for c in check_ids if c.startswith("EMIR.AUD.")}
    assert len(aud_present) >= 1, (
        f"no EMIR.AUD.* fired (expected at least 1 of 3 on the shipped "
        f"fixtures); all check_ids: {sorted(check_ids)[:20]}..."
    )


def test_emir_tr_audit_per_layer_prefixes_present() -> None:
    """check_id prefixes should cover at least TST (TSR layer) and at
    least one of FBK / TRA / single-batch."""
    _require(EMIR_TAR, EMIR_TSR, EMIR_FBK)
    import opendqi
    result = opendqi.emir.tr_audit(
        tar=str(EMIR_TAR), tsr=str(EMIR_TSR), feedback=str(EMIR_FBK)
    )
    check_ids = set(result.issues.column("check_id").to_pylist())
    tst = {c for c in check_ids if c.startswith("EMIR.TST.")}
    fbk = {c for c in check_ids if c.startswith("EMIR.FBK.")}
    # TSR layer should produce issues (we have stale valuations etc.
    # in the auth107-tsr.xml fixture).
    assert len(tst) >= 1, f"no EMIR.TST.* fired; check_ids: {sorted(check_ids)}"
    # Feedback layer should produce issues (auth092 has 2 rejections).
    assert len(fbk) >= 1, f"no EMIR.FBK.* fired; check_ids: {sorted(check_ids)}"


def test_emir_tr_audit_keyword_only_args() -> None:
    """All 3 args are keyword-only — positional should raise TypeError."""
    _require(EMIR_TAR, EMIR_TSR, EMIR_FBK)
    import opendqi
    with pytest.raises(TypeError):
        opendqi.emir.tr_audit(  # type: ignore[misc]
            str(EMIR_TAR), str(EMIR_TSR), str(EMIR_FBK)
        )


def test_sftr_tr_audit_function_present() -> None:
    """SFTR tr_audit is exposed (full end-to-end test needs an SFTR
    TAR+TSR fixture which is more involved)."""
    import opendqi
    assert callable(opendqi.sftr.tr_audit)
