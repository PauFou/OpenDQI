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
