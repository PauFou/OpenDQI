#!/usr/bin/env bash
#
# OpenDQI v0.17.0 — sftr-data-quality-pack reproducible demo.
#
# Runs the SFTR Data Quality Pack against the 5 synthetic
# ISO 20022 inputs in this directory and writes the 5
# artefacts under ./out/.
#
# v0.17 ships 16 SFTR indicators across the 5 input layers :
#
#   auth.079 (TSR)                6 DQIs (loan/collateral
#                                 missing/stale + 3 SFTR-specific:
#                                 HAIRCUT_ANOMALY, LEI_MISSING,
#                                 UNDER_COLLATERALIZATION)
#   auth.052 (TAR)                1 DQI  (TIM_REPORTING_LATE)
#   auth.080 (reconciliation)     4 DQIs (PAIRING_RATE,
#                                 RECONCILIATION_RATE,
#                                 UNPAIRED_TRADES_RATE,
#                                 FIELD_MISMATCH_RATE)
#   auth.083 (missing-collateral) 1 DQI  (MCR_OPEN_REQUESTS)
#   auth.085 (MSR T3 margin)      4 DQIs (T3_MARGIN_*) + 6
#                                 granular SFTR.T3.* checks
#
# `--as-of 2026-05-21` is pinned so the stale-loan-value /
# stale-margin cutoffs are stable across runs and the diff
# vs ./expected/ stays meaningful.
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
  --tsr                  tsr.xml \
  --tar                  tar.xml \
  --reconciliation       reconciliation.xml \
  --missing-collateral   missing_collateral.xml \
  --msr                  msr.xml \
  --as-of 2026-05-21 \
  --out out/

echo
echo "=== indicators.csv (16 rows) ==="
cat out/indicators.csv
echo
echo "Full outputs under $(pwd)/out/  (5 files)"
echo "Reference snapshot under $(pwd)/expected/ — diff to compare"
