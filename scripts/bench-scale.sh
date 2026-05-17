#!/usr/bin/env bash
# OpenDQI — end-to-end scale harness (wall-time + peak RSS).
#
# DELIBERATE, OPT-IN, LOCAL release tool. It is intentionally NOT run
# by scripts/preflight.sh or CI (those stay debug per build hygiene);
# this builds --release on purpose to measure the real product path.
# Run it by hand to (re)capture the baseline recorded in
# docs/performance.md before/after any perf work.
#
#   ./scripts/bench-scale.sh            # full: emir|sftr × 100k,1M
#   ./scripts/bench-scale.sh --smoke    # fast plumbing check (N=200)
#
# Peak RSS: this script targets macOS `/usr/bin/time -l`
# ("maximum resident set size" in BYTES). On Linux use
# `/usr/bin/time -v` ("Maximum resident set size" in KB) — swap the
# `time_cmd`/parse lines accordingly.
#
# Generated XML goes to a mktemp dir and is removed on exit — nothing
# is written inside the repo.

set -euo pipefail

cd "$(dirname "$0")/.."

SMOKE=0
[[ "${1:-}" == "--smoke" ]] && SMOKE=1

if [[ "$SMOKE" == 1 ]]; then
    SIZES=(200)
else
    SIZES=(100000 1000000)
fi

echo "→ building release binary + generator (deliberate --release; not the dev loop)"
cargo build --release -p opendqi-cli >/dev/null
cargo build --release -p opendqi-core --example gen_synthetic_xml >/dev/null

BIN=./target/release/opendqi
GEN=./target/release/examples/gen_synthetic_xml

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

echo
echo "| regime | records | wall (s) | peak RSS |"
echo "|---|---|---|---|"

for regime in emir sftr; do
    for n in "${SIZES[@]}"; do
        in="$WORK/${regime}-${n}.xml"
        out="$WORK/${regime}-${n}-out"
        tf="$WORK/${regime}-${n}.time"
        "$GEN" "$regime" "$n" "$in" >/dev/null 2>&1
        # /usr/bin/time -l writes its report to stderr; the scan's own
        # stdout/stderr are discarded so only the time report lands in $tf.
        /usr/bin/time -l "$BIN" "$regime" scan "$in" --out "$out" \
            >/dev/null 2>"$tf" || { echo "scan failed for $regime n=$n"; cat "$tf"; exit 1; }
        wall="$(awk '/ real/{print $1; exit}' "$tf")"
        rss_bytes="$(awk '/maximum resident set size/{print $1; exit}' "$tf")"
        if [[ -n "${rss_bytes:-}" ]]; then
            rss="$(awk -v b="$rss_bytes" 'BEGIN{printf "%.0f MiB", b/1048576}')"
        else
            rss="n/a"
        fi
        recs="$(grep -o '"records_processed": [0-9]*' "$out/summary.json" | awk '{print $2}')"
        printf "| %s | %s | %s | %s |\n" "$regime" "${recs:-$n}" "${wall:-n/a}" "$rss"
    done
done

echo
echo "(done — temp dir cleaned up; paste the table into docs/performance.md)"
