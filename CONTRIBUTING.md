# Contributing to OpenDQI

Thanks for your interest in contributing.

## Getting started

1. Fork and clone the repository.
2. Install Rust stable (1.75+).
3. Build the workspace:

   ```bash
   cargo build --workspace
   ```

4. Run the test suite:

   ```bash
   cargo test --workspace
   ```

## Code style

- `cargo fmt --all` before submitting.
- `cargo clippy --workspace --all-targets -- -D warnings` must pass.
- No `unwrap` / `panic` in production code paths. Use `anyhow` / `thiserror` to surface explicit errors.
- Prefer `BTreeMap` over `HashMap` when output is serialized — keeps outputs deterministic.

## Before pushing

Reproduce the CI checks locally so red runs stay rare:

```bash
cargo install cargo-deny --locked    # one-shot setup
./scripts/preflight.sh
```

`preflight.sh` runs `cargo fmt --check`, `clippy -D warnings`, `build`,
`test --workspace`, and `cargo deny check` — the same four pillars that
guard `main`. It fails fast on the first error.

For automatic enforcement on every `git push`, install the pre-push hook:

```bash
./scripts/install-hooks.sh   # symlinks .git/hooks/pre-push → scripts/git-hooks/pre-push
```

Bypass once with `git push --no-verify`; remove with `rm .git/hooks/pre-push`.

If you need a full GitHub Actions reproduction (rarely useful — preflight
covers the logic; only the action plumbing differs), install
[`nektos/act`](https://github.com/nektos/act):

```bash
brew install act                                # macOS
act -W .github/workflows/deny.yml --pull=false
```

## Tests

- Unit tests live alongside the code they exercise.
- Use only synthetic data in fixtures. Never include real regulatory or client files.
- If you add a new DQ check, include a positive and a negative test case.

## Commit messages

Use conventional, imperative commit subjects. Examples:

```
feat(core): add duplicate-UTI lifecycle awareness
fix(io): tolerate trailing whitespace in CSV headers
docs: clarify mapping YAML schema
```

## Pull requests

- One logical change per PR.
- Include test coverage for new behavior.
- Update `CHANGELOG.md` under the `## Unreleased` section.

## Scope

We welcome contributions to:

- new EMIR data-quality checks
- CSV / XML / Parquet ingestion improvements
- report templates and exports
- documentation

Out of scope for now:

- submission to Trade Repositories
- regulatory certification claims
- cloud-hosted variants

## Code of conduct

This project adopts the [Contributor Covenant v2.1](https://www.contributor-covenant.org/version/2/1/code_of_conduct/)
as its code of conduct. The full text is incorporated by
reference in [`.github/CODE_OF_CONDUCT.md`](.github/CODE_OF_CONDUCT.md);
report unacceptable behaviour privately to
`pfournier597@gmail.com`.

## Issue / PR templates

When opening an issue, GitHub will offer you a chooser between
**Bug report** and **Feature request** templates (in
[`.github/ISSUE_TEMPLATE/`](.github/ISSUE_TEMPLATE/)). Blank
issues are disabled; vulnerability reports are routed to a
private channel (see [SECURITY.md](SECURITY.md)).

PRs auto-load [`.github/PULL_REQUEST_TEMPLATE.md`](.github/PULL_REQUEST_TEMPLATE.md)
with a preflight + CHANGELOG + docs + tests checklist plus the
CLA reminder for external contributors.

## Branch protection on `main`

`main` is protected with the following ruleset (enforced via the
GitHub API, see commit `848341c+` in v0.22.0):

- **Required status checks** before merge : `Test (ubuntu-latest)`,
  `Test (macos-latest)`, `MSRV (1.87.0)`, `cargo deny`,
  `cargo llvm-cov`. All five must be green.
- **Strict** : branches must be up-to-date with `main` before
  merge (rebase or merge from main first).
- **No PR review required** — single maintainer pattern.
- **Force-push** allowed for admins only (history rewrites
  remain possible for the maintainer when needed, e.g. the
  v0.18 → v0.19 `filter-branch` operation).
- **Branch deletion** disabled.
- **enforce_admins = false** : admins can override required
  checks for emergency hotfixes.

External PRs that fail any of the 5 status checks will be
blocked until green. Internal direct-push to `main` still works
for the maintainer (admin), but the same checks run via the
push-triggered workflows and a red run signals the regression.

## License & contributions

OpenDQI is licensed under **FSL-1.1-Apache-2.0** from v0.19.0 forward
(see [LICENSE.md](LICENSE.md) and [COMPETING.md](COMPETING.md)). Each
release tag carries its own Change Date and converts automatically to
Apache-2.0 two years later. Releases v0.1.0 through v0.18.0 remain
Apache-2.0 as tagged (immutable) — they are unaffected.

### External contributors must sign the CLA

Pull requests from anyone outside the allowlist (currently `@PauFou`)
are checked by the [`cla-assistant.io`](https://cla-assistant.io) bot.
When you open your first PR, the bot will post a comment with a link
to [CLA.md](CLA.md); reply with the exact phrase shown in the comment
to sign. The status check stays red until you do — once it goes green,
your PR is eligible for review.

Signing the CLA grants the project the right to relicense your
contribution under any OSI-approved licence. This is necessary because
each FSL release auto-converts to Apache-2.0 after two years, and we
must retain the freedom to make that conversion (and any future
relicense decisions made by the project) on the entire codebase
including your contribution.

### Internal maintainers (`@PauFou`)

Allowlisted — no per-commit sign-off needed. The CLA workflow recognises
the GitHub username and skips the check.

## Release ritual

Before tagging a new release, run these steps in order on a release
branch (or directly on `main` for hotfixes):

1. Bump versions in the three manifests:
   - `Cargo.toml` (workspace.package.version)
   - `crates/opendqi-py/Cargo.toml` (package.version)
   - `crates/opendqi-py/pyproject.toml` (project.version)
2. Refresh the Cargo locks:
   ```bash
   cargo build --workspace --jobs 4
   # opendqi-py is built by maturin, not cargo from the workspace
   # root; the lock is updated by `cargo metadata` if needed (no
   # link step required) or simply left as-is when only metadata
   # fields change.
   ```
3. **Run the FSL Change Date bump** (sets `Change Date` to today+2y in
   both `LICENSE.md` and `crates/opendqi-py/LICENSE.md` atomically):
   ```bash
   bash scripts/release-license-bump.sh
   ```
4. Move the `## [Unreleased]` block in `CHANGELOG.md` to a new
   `## [X.Y.Z] - YYYY-MM-DD` section above it.
5. Commit everything with `chore(release): vX.Y.Z`, FF-merge to `main`,
   `git push`.
6. Tag and push the tag:
   ```bash
   git tag -a vX.Y.Z -m 'vX.Y.Z'
   git push origin vX.Y.Z
   ```
7. The tag push triggers two GitHub workflows in parallel:
   - `Release` (cargo-dist) → 4 CLI binaries on the GitHub Release page
   - `Python Release` (maturin) → 3 abi3 wheels uploaded to PyPI via
     OIDC trusted publisher
