#!/usr/bin/env bash
# Reproduce the same checks as CI before pushing. Run from repo root.
#
# Fails fast on the first error so you can diagnose quickly.
# cargo-deny is required by default; install it once with:
#     cargo install cargo-deny --locked
# Exceptional override (offline, debugging):
#     SKIP_DENY=1 ./scripts/preflight.sh

set -euo pipefail

echo "→ cargo fmt --all -- --check"
cargo fmt --all -- --check

echo "→ cargo clippy --workspace --all-targets -- -D warnings"
cargo clippy --workspace --all-targets -- -D warnings

echo "→ cargo build --workspace"
cargo build --workspace

echo "→ cargo test --workspace"
cargo test --workspace

if [[ "${SKIP_DENY:-0}" == "1" ]]; then
    echo "(SKIP_DENY=1 → skipping cargo deny check; CI will still run it)"
elif ! command -v cargo-deny >/dev/null 2>&1; then
    echo "✗ cargo-deny missing — required for CI parity."
    echo "  Install once with:  cargo install cargo-deny --locked"
    echo "  Or override with:   SKIP_DENY=1 ./scripts/preflight.sh"
    exit 1
else
    echo "→ cargo deny check"
    cargo deny check
fi

echo "✓ All preflight checks passed."
