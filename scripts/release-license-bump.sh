#!/usr/bin/env bash
# release-license-bump.sh — bump LICENSE.md "Change Date" to today+2y
# in both copies (repo-root LICENSE.md + crates/opendqi-py/LICENSE.md).
#
# Call this from the release commit, BEFORE `git tag`. Idempotent —
# safe to re-run; sets the same date both times on the same day.
#
# Cross-platform: detects macOS BSD `date -v+2y` vs Linux GNU
# `date -d '+2 years'` and uses whichever is available, so the script
# is identical on the dev box (macOS) and the GitHub Actions Ubuntu
# runner.
#
# Why two files: the Python wheel needs LICENSE.md sitting next to
# its pyproject.toml at build time (maturin's working directory is
# crates/opendqi-py/, not the repo root). Symlinks were rejected
# because they break wheel builds on Windows and complicate maturin's
# source distribution generation.

set -euo pipefail

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

if date -v+2y -u +%Y-%m-%d >/dev/null 2>&1; then
    # macOS / BSD date
    CHANGE_DATE=$(date -v+2y -u +%Y-%m-%d)
else
    # Linux / GNU date
    CHANGE_DATE=$(date -u -d '+2 years' +%Y-%m-%d)
fi

FILES=(LICENSE.md crates/opendqi-py/LICENSE.md)
for f in "${FILES[@]}"; do
    if [ ! -f "$f" ]; then
        echo "✗ missing: $f" >&2
        exit 1
    fi
    sed -i.bak -E "s/^(\| Change Date *\| *)[0-9]{4}-[0-9]{2}-[0-9]{2}( *\|)/\1$CHANGE_DATE\2/" "$f"
    rm -f "$f.bak"
done

echo "✓ Change Date set to $CHANGE_DATE in:"
printf '    %s\n' "${FILES[@]}"
echo
echo "Reminder: this script is part of the release ritual. After"
echo "running it, complete the release commit and tag:"
echo "  git add LICENSE.md crates/opendqi-py/LICENSE.md"
echo "  git commit -m 'chore(release): vX.Y.Z'"
echo "  git tag -a vX.Y.Z -m 'vX.Y.Z'"
