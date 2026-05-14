#!/usr/bin/env bash
# Reproduce the same checks as CI before pushing. Run from repo root.
#
# Fails fast on the first error so you can diagnose quickly.
# `cargo deny` is invoked when the binary is installed locally;
# otherwise we skip and rely on CI to catch the audit failures.

set -euo pipefail

echo "→ cargo fmt --all -- --check"
cargo fmt --all -- --check

echo "→ cargo clippy --workspace --all-targets -- -D warnings"
cargo clippy --workspace --all-targets -- -D warnings

echo "→ cargo build --workspace"
cargo build --workspace

echo "→ cargo test --workspace"
cargo test --workspace

if command -v cargo-deny >/dev/null 2>&1; then
    echo "→ cargo deny check"
    cargo deny check
else
    echo "(skipping cargo deny — install with: cargo install cargo-deny)"
fi

echo "✓ All preflight checks passed."
