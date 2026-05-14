#!/usr/bin/env bash
# Install git hooks for this repo. Idempotent.
#
#   ./scripts/install-hooks.sh
#
# Currently installs:
#   pre-push  → runs ./scripts/preflight.sh on every push.
#
# Bypass for a single push: git push --no-verify
# Remove the hook:          rm .git/hooks/pre-push

set -euo pipefail

root="$(git rev-parse --show-toplevel)"
hooks_dir="$root/.git/hooks"

mkdir -p "$hooks_dir"
ln -sfn "../../scripts/git-hooks/pre-push" "$hooks_dir/pre-push"

echo "✓ Pre-push hook installed → runs ./scripts/preflight.sh on every push."
echo "  Bypass once with:  git push --no-verify"
echo "  Remove with:       rm $hooks_dir/pre-push"
