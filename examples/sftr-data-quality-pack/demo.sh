#!/usr/bin/env bash
#
# OpenDQI v0.16.0 — sftr-data-quality-pack reproducible demo.
#
# Runs the SFTR Data Quality Pack against the 2 synthetic
# ISO 20022 inputs in this directory (auth.079 TSR +
# auth.052 TAR) and writes the 5 artefacts under ./out/.
#
# v0.16 ships 4 SFTR indicators on the T2 layer of
# auth.079 + auth.052. T3 margin + auth.080 reconciliation +
# auth.083 missing-collateral are scheduled for v0.17 and
# would expand this output once those computers ship.
#
# `--as-of 2026-05-21` is pinned so the stale-loan-value
# cutoff is stable across runs and the diff vs ./expected/
# stays meaningful.
#
# Re-run after upstream Rust changes:
#   diff out/indicators.csv expected/indicators.csv
# should stay empty unless a DQI formula intentionally changed.

set -euo pipefail
cd "$(dirname "$0")"

# Build the debug binary if missing (debug only — per the
# project's build hygiene rules, we never run release builds
# in dev/demo paths).
if [[ ! -x ../../target/debug/opendqi ]]; then
  (cd ../.. && cargo build --bin opendqi --jobs 4)
fi

rm -rf out/
../../target/debug/opendqi sftr data-quality-pack \
  --tsr tsr.xml \
  --tar tar.xml \
  --as-of 2026-05-21 \
  --out out/

echo
echo "=== indicators.csv (4 rows) ==="
cat out/indicators.csv
echo
echo "Full outputs under $(pwd)/out/  (5 files)"
echo "Reference snapshot under $(pwd)/expected/ — diff to compare"
