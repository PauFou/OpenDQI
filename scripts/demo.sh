#!/usr/bin/env bash
# OpenDQI — 30-second walkthrough of the three primary EMIR workflows.
#
# Runs against the synthetic kit at examples/quickstart-emir/ and
# drops three HTML reports under /tmp/opendqi-demo/. On macOS it
# opens the first report automatically; on Linux it tries xdg-open.
# Both fall back to a plain `echo` if no opener is available.
#
# Builds the CLI in DEBUG (--jobs 4) on first run only — never
# touches the release profile (CLAUDE.md build hygiene).
#
# Compatible with macOS bash 3.2 and modern bash; `set -u` safe.

set -euo pipefail

# Resolve repo root (one directory up from this script) so the
# demo can be invoked from anywhere — e.g. `bash scripts/demo.sh`
# from $HOME or `./scripts/demo.sh` from the repo root.
SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd -- "$SCRIPT_DIR/.." && pwd)
cd "$REPO_ROOT"

KIT="examples/quickstart-emir"
OUT_BASE="/tmp/opendqi-demo"
BIN="target/debug/opendqi"

if [ ! -d "$KIT" ]; then
    echo "✗ Missing $KIT — run from a checkout of the OpenDQI repo." >&2
    exit 1
fi

# Build only if the binary is missing or older than its sources.
# We keep this incremental — never `cargo clean`, never --release.
if [ ! -x "$BIN" ] || \
   [ -n "$(find crates -name '*.rs' -newer "$BIN" -print -quit 2>/dev/null)" ]; then
    echo "→ Building opendqi (debug, --jobs 4)…"
    cargo build -p opendqi-cli --jobs 4
fi

rm -rf "$OUT_BASE"
mkdir -p "$OUT_BASE/tsr" "$OUT_BASE/feedback" "$OUT_BASE/audit"

echo
echo "─── 1/3 · TR state health (auth.107) ─────────────────────────"
"$BIN" emir tr-state-scan \
    "$KIT/auth107-tsr.xml" \
    --out "$OUT_BASE/tsr/"

echo
echo "─── 2/3 · Rejection intelligence (auth.092) ──────────────────"
"$BIN" emir feedback \
    "$KIT/auth092-feedback.xml" \
    --store "$OUT_BASE/history.db" \
    --out "$OUT_BASE/feedback/"

echo
echo "─── 3/3 · Combined audit (auth.030 + auth.107 + auth.092) ────"
"$BIN" emir tr-audit \
    --tar      "$KIT/auth030-tar.xml" \
    --tsr      "$KIT/auth107-tsr.xml" \
    --feedback "$KIT/auth092-feedback.xml" \
    --out "$OUT_BASE/audit/"

echo
echo "✓ All three reports written under $OUT_BASE/"
echo
echo "   • $OUT_BASE/tsr/tr_state_report.html"
echo "   • $OUT_BASE/feedback/report.html"
echo "   • $OUT_BASE/audit/tr_audit_report.html"
echo

# Try to open the headline report (audit = the most visually rich).
TARGET="$OUT_BASE/audit/tr_audit_report.html"
if command -v open >/dev/null 2>&1; then
    echo "→ Opening $TARGET (macOS)…"
    open "$TARGET"
elif command -v xdg-open >/dev/null 2>&1; then
    echo "→ Opening $TARGET (Linux)…"
    xdg-open "$TARGET" >/dev/null 2>&1 &
else
    echo "(No GUI opener found — open the file manually in a browser.)"
fi
