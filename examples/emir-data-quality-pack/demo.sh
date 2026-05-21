#!/usr/bin/env bash
#
# OpenDQI v0.16.0 — emir-data-quality-pack reproducible demo.
#
# Runs the full EMIR Data Quality Pack against the 5
# synthetic ISO 20022 inputs in this directory and writes the
# 5 artefacts (report.html + summary.json + issues.csv +
# indicators.csv + evidence.csv) under ./out/.
#
# v0.16 ships 24 EMIR indicators (10 v0.15 + 14 new in v0.16).
# 7 of the new auth.091-derived cross-CP DQIs self-report
# `not_applicable` here because the CLI flag (--recon-stats /
# --reconciliation) is not yet wired on `data-quality-pack`
# — the computers exist in the core engine and will fire once
# those flags are threaded.
#
# `--as-of 2026-05-21` is pinned so the stale-valuation /
# stale-collateral-state cutoffs are stable across runs and
# the diff vs ./expected/ stays meaningful.
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
../../target/debug/opendqi emir data-quality-pack \
  --tsr tsr.xml \
  --tar tar.xml \
  --msr msr.xml \
  --mar mar.xml \
  --feedback feedback.xml \
  --as-of 2026-05-21 \
  --out out/

echo
echo "=== indicators.csv (24 rows) ==="
cat out/indicators.csv
echo
echo "Full outputs under $(pwd)/out/  (5 files)"
echo "Reference snapshot under $(pwd)/expected/ — diff to compare"
