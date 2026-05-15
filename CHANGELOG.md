# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-05-15

Feature release — backwards-compatible additions on top of v0.1.0.

### Added

- **Structured evidence in the HTML report.** `report.html` renders
  a collapsible `evidence` block (Field / Before / After / Line) per
  issue that carries it — the audit trail captured on lifecycle,
  reconciliation, duplicate-UTI and book-vs-TSR checks is now visible
  in the primary human-facing artifact, not only in `issues.csv`.
- **`opendqi completions <shell>`** — generates shell completion
  scripts for bash / zsh / fish / powershell / elvish (stdout).
- **`opendqi man`** — renders the top-level man page (roff) to
  stdout.
- **Book-vs-TSR reconciliation in the local web UI** — multi-file
  upload (book CSV + TSR XML + mapping YAML) for EMIR and SFTR.
- **TR audit in the local web UI** — multi-file upload (TAR + TSR +
  feedback XML) running every per-layer check pack plus the three
  cross-layer `*.AUD.*` coherence checks. The desktop UI now covers
  10 operations — full parity with every report-producing CLI flow.
  (Web UI runs without a history store; store-backed lifecycle
  checks remain CLI-only.)

### Changed

- `compute_book_reconcile_issues` / `compute_sftr_book_reconcile_issues`
  and `compute_tr_audit_emir_issues` / `compute_tr_audit_sftr_issues`
  hoisted into `opendqi-core` as pure, unit-tested functions. The
  CLI and web UI now share a single implementation each — no
  duplicated reconciliation / audit logic.
- Workspace version `0.1.0` → `0.2.0`.

## [0.1.0] - 2026-05-15

First tagged release. OpenDQI is a local-first data-quality engine
for EMIR and SFTR regulatory reporting files: it ingests both the
reports a firm submits to its Trade Repository and the files the TR
sends back, and turns them into reproducible HTML / JSON / CSV
(and Parquet) outputs.

### Added

- **Canonical domain model** — `EmirRecord`, `SftrRecord`,
  `TrStateRecord`, `SftrTrStateRecord`, `MarginActivityRecord`,
  `MarginStateRecord`, `ReconciliationRecord`, `ReconStatsRecord`,
  `FeedbackRecord`, `RejectionProfile`, `DqIssue` (with structured
  `evidence: Vec<EvidenceItem>`), `ScanSummary`. Heavy use of
  `Option<T>` and a `raw_fields` catch-all per record.
- **199 reproducible data-quality checks** (135 EMIR + 64 SFTR)
  across the six DQ dimensions — completeness, validity, accuracy,
  consistency, uniqueness, timeliness — plus dedicated TR-layer
  families: TSR state-health, TAR activity, MAR/MSR margin,
  cross-batch lifecycle, feedback, reconciliation, book-vs-TSR,
  `EMIR.RST.*` reconciliation statistics (auth.091), and the
  `EMIR.PSC.*` / `SFTR.PSC.*` pre-submission families that flag
  records likely to be rejected based on observed TR feedback.
- **ISO 20022 ingestion** — EMIR `auth.030` (TAR), `auth.107`
  (TSR), `auth.092` (feedback), `auth.106` (reconciliation,
  synthetic schema), `auth.108` (MAR), `auth.109` (MSR), `auth.091`
  (reconciliation statistics); SFTR `auth.052` (TAR), `auth.079`
  (TSR), `auth.080` (feedback), `auth.083` (reconciliation). CSV
  ingestion with a YAML mapping; Parquet read + write round-trip.
  Optional XSD validation via `xmllint`.
- **CLI** — `opendqi {emir,sftr} scan / validate / feedback /
  reconcile / tr-state-scan / tr-activity-scan / tr-audit /
  book-reconcile / normalize`, `opendqi emir {mar-scan, msr-scan,
  recon-stats}`, the store-side `opendqi feedback
  list/resolve/stale/analytics` workflow, `opendqi desktop`, and
  `opendqi smtp-test` for SMTP-config validation.
- **Post-TR → pre-TR feedback loop** — `opendqi feedback
  analytics` exports `rejection_profile.yml`; passing it back via
  `--rejection-profile` on `{emir,sftr} scan` runs the `*.PSC.*`
  family so historical rejection patterns inform the next scan.
- **Local SQLite history store** (opt-in `--store`) persisting
  submissions, feedback rows, and reconciliation rows, enabling
  cross-batch lifecycle checks and the Open/Resolved/Stale feedback
  workflow.
- **Local web UI** (`opendqi desktop`, binds `127.0.0.1:7878`) with
  8 drag-and-drop operations: scan, tr-state-scan, tr-activity-scan,
  feedback, recon-stats, mar-scan, msr-scan, validate.
- **Email notifications** — `--email-config <yml>` on every
  report-producing command (15 total). SMTP password is read from
  an environment variable, never stored in YAML. Built on `lettre`
  with `rustls-tls` (no OpenSSL link).
- **Canonical-model completeness** — `EmirRecord.source_system`,
  `SftrRecord.security_identifier`, `DqIssue.evidence`,
  `Thresholds.severity_overrides` (per-check-id YAML overrides
  applied at a single chokepoint).
- **Deterministic outputs** — `summary.json`, `issues.csv` (with
  `evidence_json`), `report.html`, plus per-layer artefacts.
  Parallel check execution via `rayon`.
- **Infrastructure** — GitHub Actions CI (fmt / clippy / build /
  test on Ubuntu + macOS), daily `cargo-deny` security audit,
  `scripts/preflight.sh`, opt-in pre-push hook via
  `scripts/install-hooks.sh`. 625 tests.

### Changed

- Positioning: OpenDQI is a post-TR feedback / TAR-TSR state-health
  / regulatory data-quality engine, not merely a pre-submission XML
  validator. The pre-submission layer is now *informed* by observed
  rejection patterns.
- All 20 `run_all*` runners share a single `finalize_issues`
  chokepoint (severity overrides + deterministic sort).

### Security

- Workspace crates are `publish = false`. `cargo-deny` enforces an
  allow-list of permissive licenses (MIT / Apache-2.0 / BSD / ISC /
  Unicode-3.0 / CC0-1.0 / 0BSD / …) and rejects unknown registries.
- No SWIFT-licensed XSDs or real client data are committed; all
  fixtures are synthetic.

[Unreleased]: https://github.com/PauFou/OpenDQI/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/PauFou/OpenDQI/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/PauFou/OpenDQI/releases/tag/v0.1.0
