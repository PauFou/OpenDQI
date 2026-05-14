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

## License

By contributing, you agree that your contributions will be licensed under the Apache License, Version 2.0.
