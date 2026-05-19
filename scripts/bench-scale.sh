#!/usr/bin/env bash
# OpenDQI — end-to-end scale harness (wall-time + peak RSS).
#
# DELIBERATE, OPT-IN, LOCAL release tool. It is intentionally NOT run
# by scripts/preflight.sh or CI (those stay debug per build hygiene);
# this builds --release on purpose to measure the real product path.
# Run it by hand to (re)capture the baseline recorded in
# docs/performance.md before/after any perf work.
#
#   ./scripts/bench-scale.sh                # full: emir|sftr × 100k,1M
#   ./scripts/bench-scale.sh --smoke        # fast plumbing check (N=200)
#   ./scripts/bench-scale.sh --mem-trace    # also print per-phase RSS
#
# --mem-trace exports OPENDQI_MEM_TRACE=1 so the scan emits
# `MEMTRACE phase=<p> rss_bytes=<n>` lines on stderr at the scan
# pipeline's phase boundaries (discovery/parse/checks/lifecycle+
# presub/finalize/report). This script then prints a second table
# attributing peak RSS to a phase. Opt-in only; the env var is unset
# in normal use / preflight / CI so behaviour is byte-unchanged.
# Flags compose and are order-independent.
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
MEMTRACE=0
for arg in "$@"; do
    case "$arg" in
        --smoke) SMOKE=1 ;;
        --mem-trace) MEMTRACE=1 ;;
        *) echo "unknown arg: $arg (use --smoke and/or --mem-trace)"; exit 2 ;;
    esac
done

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

if [[ "$MEMTRACE" == 1 ]]; then
    MT_ENV=(env OPENDQI_MEM_TRACE=1)
else
    MT_ENV=()
fi
PHASE_ROWS=""
PEAK_ROWS=""

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
        # stdout/stderr are discarded so only the time report (and, with
        # --mem-trace, the scan's MEMTRACE lines) land in $tf.
        ${MT_ENV[@]+"${MT_ENV[@]}"} /usr/bin/time -l "$BIN" "$regime" scan "$in" --out "$out" \
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
        if [[ "$MEMTRACE" == 1 ]]; then
            # Phase rows in execution order (awk preserves file order).
            rows="$(awk -v r="$regime" -v n="${recs:-$n}" '
                /^MEMTRACE phase=/ {
                    split($2, a, "="); ph = a[2]
                    split($3, b, "="); rb = b[2]
                    if (rb == "unknown") v = "n/a"
                    else v = sprintf("%.0f MiB", rb / 1048576)
                    printf "| %s | %s | %s | %s |\n", r, n, ph, v
                }' "$tf")"
            PHASE_ROWS+="$rows"$'\n'
            # True peak: the sampler thread's last line catches a
            # transient that spikes/frees *between* boundary samples.
            prow="$(awk -v r="$regime" -v n="${recs:-$n}" '
                /^MEMTRACE peak / {
                    split($3, a, "="); rb = a[2]
                    split($4, b, "="); ms = b[2]
                    split($5, c, "="); ph = c[2]
                    if (rb == "unknown") v = "n/a"
                    else v = sprintf("%.0f MiB", rb / 1048576)
                    printf "| %s | %s | %s | %s | %s |\n", r, n, v, ms, ph
                }' "$tf")"
            PEAK_ROWS+="$prow"$'\n'
        fi
    done
done

if [[ "$MEMTRACE" == 1 ]]; then
    echo
    echo "Phase attribution (current RSS sampled at each scan boundary;"
    echo "coarse — see docs/performance.md):"
    echo
    echo "| regime | records | phase | RSS |"
    echo "|---|---|---|---|"
    printf '%s' "$PHASE_ROWS" | grep -v '^$' || true
    echo
    echo "True-peak attribution (background sampler; catches a"
    echo "transient freed *between* boundaries — see docs/performance.md):"
    echo
    echo "| regime | records | peak RSS | at (ms) | phase live at peak |"
    echo "|---|---|---|---|---|"
    printf '%s' "$PEAK_ROWS" | grep -v '^$' || true
fi

echo
echo "(done — temp dir cleaned up; paste the table into docs/performance.md)"
