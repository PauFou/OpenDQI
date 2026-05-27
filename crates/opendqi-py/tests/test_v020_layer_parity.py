"""
v0.20 — Python + CLI parity tests for the per-layer wrappers
shipped in Phase B (5 SFTR CLI subcommands + 2 EMIR CLI
subcommands) and Phase C (7 EMIR Python wrappers).

Two distinct test groups:

  (a) EMIR Python wrappers (7 functions):
        opendqi.emir.mar_scan       — auth.108
        opendqi.emir.msr_scan       — auth.109
        opendqi.emir.recon_stats    — auth.091 (+ optional prior=)
        opendqi.emir.warnings       — auth.106
        opendqi.emir.feedback       — auth.092
        opendqi.emir.query_scan     — auth.029 (envelope-only)
        opendqi.emir.status_advice_scan — auth.031 (envelope-only)

  (b) SFTR CLI smoke (5 subcommands) via subprocess:
        opendqi sftr mar-scan              — auth.070
        opendqi sftr msr-scan              — auth.085
        opendqi sftr tr-status-advice-scan — auth.084
        opendqi sftr reuse-activity-scan   — auth.071
        opendqi sftr reuse-state-scan      — auth.086

  Each assertion is intentionally weak (smoke-shape only —
  records>=0, score is a float) because exact issue counts are
  governed by the Rust-side check tests; this file is the
  Python/CLI surface validation, not the check correctness gate.
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

import pytest


REPO_ROOT = Path(__file__).resolve().parents[3]

# EMIR fixtures (for Python wrapper tests)
EMIR_FIXTURES = {
    "mar": REPO_ROOT / "examples" / "emir" / "mar" / "auth108-sample.xml",
    "msr": REPO_ROOT / "examples" / "emir" / "msr" / "auth109-sample.xml",
    "recon_stats": REPO_ROOT / "examples" / "emir" / "recon_stats" / "auth091-sample.xml",
    "recon_stats_prior": REPO_ROOT / "examples" / "emir" / "recon_stats" / "auth091-prior.xml",
    "warnings": REPO_ROOT / "examples" / "emir" / "warnings" / "auth106-sample.xml",
    "feedback": REPO_ROOT / "examples" / "emir" / "feedback" / "auth092-sample.xml",
    "query": REPO_ROOT / "examples" / "emir" / "query" / "auth029-sample.xml",
    "status_advice": REPO_ROOT / "examples" / "emir" / "status_advice" / "auth031-sample.xml",
}

# SFTR fixtures (for CLI smoke tests)
SFTR_FIXTURES = {
    "mar": REPO_ROOT / "examples" / "sftr" / "margin_activity" / "auth070-sample.xml",
    "msr": REPO_ROOT / "examples" / "sftr" / "margin_state" / "auth085-sample.xml",
    "tsa": REPO_ROOT / "examples" / "sftr" / "tr_status_advice" / "auth084-sample.xml",
    "reu_a": REPO_ROOT / "examples" / "sftr" / "reuse_activity" / "auth071-sample.xml",
    "reu_s": REPO_ROOT / "examples" / "sftr" / "reuse_state" / "auth086-sample.xml",
}


def _require(*paths: Path) -> None:
    missing = [p for p in paths if not p.exists()]
    if missing:
        pytest.skip(f"fixtures missing: {missing}")


def _opendqi_bin() -> Path | None:
    """Locate the opendqi CLI binary (debug or release)."""
    for sub in ("debug", "release"):
        p = REPO_ROOT / "target" / sub / "opendqi"
        if p.exists():
            return p
    # Fall back to `which opendqi` so a `cargo install`'d binary works.
    found = shutil.which("opendqi")
    return Path(found) if found else None


# =================================================================
# (a) EMIR Python wrappers — 7 functions, 1 test each
# =================================================================


def _assert_pyscan_shape(result, regime: str, min_records: int = 0):
    """Smoke-validate PyScanResult.summary shape."""
    s = result.summary
    assert s["regime"] == regime, s
    assert s["files_processed"] == 1, s
    assert s["records_processed"] >= min_records, s
    assert isinstance(s["issues_total"], int), s
    assert isinstance(s["quality_score"], (int, float)), s
    # issues is a pyarrow.Table | None
    if result.issues is not None:
        assert result.issues.num_rows == s["issues_total"], (result.issues, s)


def test_emir_mar_scan():
    _require(EMIR_FIXTURES["mar"])
    import opendqi
    result = opendqi.emir.mar_scan(str(EMIR_FIXTURES["mar"]))
    _assert_pyscan_shape(result, "emir", min_records=1)


def test_emir_msr_scan():
    _require(EMIR_FIXTURES["msr"])
    import opendqi
    result = opendqi.emir.msr_scan(str(EMIR_FIXTURES["msr"]))
    _assert_pyscan_shape(result, "emir", min_records=1)


def test_emir_recon_stats_no_prior():
    _require(EMIR_FIXTURES["recon_stats"])
    import opendqi
    result = opendqi.emir.recon_stats(str(EMIR_FIXTURES["recon_stats"]))
    _assert_pyscan_shape(result, "emir", min_records=1)


def test_emir_recon_stats_with_prior_kwarg():
    _require(EMIR_FIXTURES["recon_stats"], EMIR_FIXTURES["recon_stats_prior"])
    import opendqi
    result = opendqi.emir.recon_stats(
        str(EMIR_FIXTURES["recon_stats"]),
        prior=str(EMIR_FIXTURES["recon_stats_prior"]),
    )
    _assert_pyscan_shape(result, "emir", min_records=1)


def test_emir_warnings():
    _require(EMIR_FIXTURES["warnings"])
    import opendqi
    result = opendqi.emir.warnings(str(EMIR_FIXTURES["warnings"]))
    _assert_pyscan_shape(result, "emir", min_records=1)


def test_emir_feedback():
    _require(EMIR_FIXTURES["feedback"])
    import opendqi
    result = opendqi.emir.feedback(str(EMIR_FIXTURES["feedback"]))
    _assert_pyscan_shape(result, "emir", min_records=1)


def test_emir_query_scan_envelope_only():
    """auth.029 has 0 business DQ signal — only EMIR.QRY.ENVELOPE_WELLFORMED fires."""
    _require(EMIR_FIXTURES["query"])
    import opendqi
    result = opendqi.emir.query_scan(str(EMIR_FIXTURES["query"]))
    _assert_pyscan_shape(result, "emir", min_records=1)
    # Fully-populated envelope -> no issues, perfect score.
    assert result.summary["issues_total"] == 0
    assert result.summary["quality_score"] == 100.0


def test_emir_status_advice_scan_envelope_only():
    """auth.031 has 0 business DQ signal — only EMIR.ACK.ENVELOPE_WELLFORMED fires."""
    _require(EMIR_FIXTURES["status_advice"])
    import opendqi
    result = opendqi.emir.status_advice_scan(str(EMIR_FIXTURES["status_advice"]))
    _assert_pyscan_shape(result, "emir", min_records=1)
    assert result.summary["issues_total"] == 0
    assert result.summary["quality_score"] == 100.0


def test_emir_module_lists_all_v020_entry_points():
    """Surface-level guarantee: every v0.20 EMIR wrapper is registered on the module."""
    import opendqi
    for fn_name in (
        "mar_scan",
        "msr_scan",
        "recon_stats",
        "warnings",
        "feedback",
        "query_scan",
        "status_advice_scan",
    ):
        assert hasattr(opendqi.emir, fn_name), f"opendqi.emir.{fn_name} missing"


# =================================================================
# (b) SFTR CLI smoke — 5 subcommands, 1 test each
# =================================================================


def _run_sftr_cli(subcommand: str, fixture: Path, output_prefix: str, tmp_path: Path):
    """Invoke the opendqi CLI; assert exit 0 + summary.json + the
    layer-specific issues.csv exist with the right shape."""
    bin_path = _opendqi_bin()
    if bin_path is None:
        pytest.skip(
            "opendqi CLI binary not found under target/{debug,release}/opendqi; "
            "run `cargo build` from the workspace root, or `cargo install` it."
        )
    cmd = [str(bin_path), "sftr", subcommand, str(fixture), "--out", str(tmp_path)]
    proc = subprocess.run(cmd, capture_output=True, text=True, timeout=60)
    assert proc.returncode == 0, (
        f"{' '.join(cmd)} exited {proc.returncode}\n"
        f"stdout:\n{proc.stdout}\nstderr:\n{proc.stderr}"
    )
    summary_path = tmp_path / "summary.json"
    issues_path = tmp_path / f"{output_prefix}_issues.csv"
    assert summary_path.exists(), f"missing {summary_path}"
    assert issues_path.exists(), f"missing {issues_path}"
    summary = json.loads(summary_path.read_text())
    assert summary["regime"] == "sftr", summary
    assert summary["records_processed"] >= 0, summary
    return summary


def test_sftr_mar_scan_cli(tmp_path):
    _require(SFTR_FIXTURES["mar"])
    _run_sftr_cli("mar-scan", SFTR_FIXTURES["mar"], "mar", tmp_path)


def test_sftr_msr_scan_cli(tmp_path):
    _require(SFTR_FIXTURES["msr"])
    _run_sftr_cli("msr-scan", SFTR_FIXTURES["msr"], "msr", tmp_path)


def test_sftr_tr_status_advice_scan_cli(tmp_path):
    _require(SFTR_FIXTURES["tsa"])
    _run_sftr_cli(
        "tr-status-advice-scan",
        SFTR_FIXTURES["tsa"],
        "tr_status_advice",
        tmp_path,
    )


def test_sftr_reuse_activity_scan_cli(tmp_path):
    _require(SFTR_FIXTURES["reu_a"])
    _run_sftr_cli(
        "reuse-activity-scan",
        SFTR_FIXTURES["reu_a"],
        "reuse_activity",
        tmp_path,
    )


def test_sftr_reuse_state_scan_cli(tmp_path):
    _require(SFTR_FIXTURES["reu_s"])
    _run_sftr_cli(
        "reuse-state-scan",
        SFTR_FIXTURES["reu_s"],
        "reuse_state",
        tmp_path,
    )
