<!--
Thanks for opening a PR against OpenDQI. Filling this out helps
the reviewer scope your change quickly.
-->

## Summary

<!-- One paragraph: what changes, why. Link the related issue
     if any (e.g. `Closes #42`). -->

## Type of change

- [ ] Bug fix
- [ ] New feature (check / DQI / parser / CLI / Python)
- [ ] Refactor (no behaviour change)
- [ ] Documentation
- [ ] Test / golden / fixture
- [ ] CI / release / process
- [ ] Other (describe)

## Checklist

- [ ] `bash scripts/preflight.sh` passes locally
  (`cargo fmt --check`, `clippy -D warnings`, `build`, `test`,
  `deny check`).
- [ ] `CHANGELOG.md` updated under the `## [Unreleased]` section
  (or this PR is part of a release commit that moves it).
- [ ] Relevant docs updated (`docs/`, `README.md`, per-message
  notes in `docs/auth-messages/` if a parser changed).
- [ ] New / changed tests cover the behaviour.
- [ ] No SWIFT-licensed XSDs, no real counterparty data, no
  ESMA-internal file paths committed (project confidentiality
  rule — see `CONTRIBUTING.md`).

## CLA reminder (external contributors)

If this is your first PR to OpenDQI and you are not on the
allowlist, the `cla-assistant.io` bot will post a comment
shortly after this PR opens with a link to
[CLA.md](https://github.com/PauFou/OpenDQI/blob/main/CLA.md).
Reply with the exact phrase the bot specifies to sign. The
status check stays red until you do. Signing the CLA grants
the project the right to relicense your contribution under
any OSI-approved licence, which is necessary for the
FSL-1.1-Apache-2.0 auto-conversion mechanism. See
[CONTRIBUTING.md#license--contributions](https://github.com/PauFou/OpenDQI/blob/main/CONTRIBUTING.md#license--contributions).

## Additional context

<!-- Anything else useful for the reviewer: design alternatives
     considered, performance numbers if a perf change, screenshots
     if a report-rendering change, etc. -->
