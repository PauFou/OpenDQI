#!/usr/bin/env python3
"""
OpenDQI Python quickstart — Pattern 4 (v0.13.0): cross-message
tr_audit.

Three files, one call — runs every layer's checks PLUS the 3
EMIR.AUD.* cross-layer coherence checks (rejected-but-
outstanding-in-TSR, NEWT-in-TAR-not-in-TSR, OUTSTANDING-IN-TSR-
NOT-IN-TAR). The single most powerful Python entry point
for post-TR intelligence.

Run from the repo root:

    pip install opendqi
    python examples/python/04_tr_audit.py
"""
from __future__ import annotations

import json
from collections import Counter
from pathlib import Path

import opendqi


REPO_ROOT = Path(__file__).resolve().parents[2]
TAR = REPO_ROOT / "examples" / "quickstart-emir" / "auth030-tar.xml"
TSR = REPO_ROOT / "examples" / "quickstart-emir" / "auth107-tsr.xml"
FBK = REPO_ROOT / "examples" / "quickstart-emir" / "auth092-feedback.xml"


def main() -> None:
    for path in (TAR, TSR, FBK):
        if not path.exists():
            raise SystemExit(f"✗ fixture missing: {path}")

    print(f"tr_audit on:")
    print(f"  TAR (auth.030)      → {TAR.name}")
    print(f"  TSR (auth.107)      → {TSR.name}")
    print(f"  feedback (auth.092) → {FBK.name}")

    result = opendqi.emir.tr_audit(tar=str(TAR), tsr=str(TSR), feedback=str(FBK))

    print("\n=== summary (all 3 layers + 3 cross-layer EMIR.AUD.*) ===")
    print(json.dumps(result.summary, indent=2, default=str))

    print("\n=== top 5 check_ids by frequency (per-layer breakdown) ===")
    counts = Counter(result.issues.column("check_id").to_pylist())
    for check_id, n in counts.most_common(5):
        prefix = check_id.split(".")[1]  # COMP / TST / FBK / AUD / ...
        print(f"  {n:>3}× {check_id}  ({prefix} layer)")

    # Filter on just the AUD cross-layer to see what only this
    # entry point can produce.
    print("\n=== cross-layer EMIR.AUD.* (only tr_audit produces these) ===")
    aud_only = [c for c in counts if c.startswith("EMIR.AUD.")]
    if not aud_only:
        print("  (none fired — fixtures are well-aligned across layers)")
    for check_id in sorted(aud_only):
        print(f"  {counts[check_id]:>3}× {check_id}")


if __name__ == "__main__":
    main()
