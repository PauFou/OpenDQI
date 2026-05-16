# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Faithful EMIR `auth.106` data-quality warnings.** Real
  `auth.106.001.01` is a *Derivatives Trade Warnings Report* (ESMA
  **DATWRN**) — aggregate missing-valuation / missing-margin-info /
  abnormal-values statistics, **not** a counterparty pairing report.
  A schema-aligned parser (`crates/opendqi-xml/src/emir_warnings.rs`)
  reads the real envelope and derives the report-level rates onto a
  new `TradeWarningsRecord`; `opendqi emir warnings` (CLI and the
  local web UI's warnings operation — shared core) runs 5 new
  `EMIR.WRN.*` threshold checks (missing/outdated valuation & margin,
  abnormal values) with configurable `WarningsThresholds`.
  `DataSetActn=NOTX` → `EMIR.FMT.WRN_NO_RECORDS`. The per-counterparty
  `Wrnngs` breakdown is a documented deferred subset. Covered by
  inline + integration + golden + robustness tests. See
  [`docs/auth-messages/emir-auth106.md`](docs/auth-messages/emir-auth106.md)
  and [`docs/emir-warnings.md`](docs/emir-warnings.md). (The legacy
  *synthetic* pairing path mislabelled `auth.106`/`auth.083` is
  removed in the next increment.)
- **Golden snapshot regression harness.** A dependency-free
  integration test (`crates/opendqi-cli/tests/golden.rs`) runs the
  real `opendqi` binary over the synthetic `examples/` fixtures for
  every report-producing command family (17 cases) and pins the
  deterministic `summary.json` + issues CSV byte-for-byte against
  committed goldens, normalizing only absolute paths (→ `<WS>` /
  `<TMP>`) and wall-clock timestamps. Locks the
  "deterministic outputs" product guarantee against regressions.
  Regenerate with `UPDATE_GOLDEN=1`. See
  [`docs/reliability.md`](docs/reliability.md).
- **Parser robustness suite.** Dependency-free, fixed-seed adversarial
  tests (`crates/opendqi-xml/tests/robustness.rs`,
  `crates/opendqi-io/tests/robustness_io.rs`) drive every public
  parser / ingestion entry point with a hostile corpus (empty,
  malformed, truncated, invalid-UTF-8, wrong-namespace, deep-nesting,
  size bombs, billion-laughs, garbage zip/gzip/Parquet, hostile
  CSV/YAML) plus deterministic byte-mutation of the valid fixtures,
  asserting each call returns `Ok`/`Err` and **never panics or
  exceeds a 15s wall-clock bound**. Includes a self-test proving the
  harness actually catches an injected panic. No parser change was
  needed — the streaming parsers already survive the full corpus.

### Changed

- **`unwrap()`/`expect()` audit of the parse paths.** Audited every
  non-test `unwrap()`/`expect()` in `opendqi-xml`, the `opendqi-io`
  ingestion readers and the `opendqi-core` conversion helpers: the
  untrusted parse/ingest paths contain **none** (all fallible
  conversions are already graceful — `.ok()`/`?`/`DqIssue`). The only
  non-test occurrences are in the Parquet *writer* / `Default` impls
  over crate constants, not input; the lone un-annotated one
  (`decimal_builder`) gained an inline justification. No behavior
  change. The panic-freedom invariant is now documented in
  [`docs/reliability.md`](docs/reliability.md).

### Removed

## [0.4.0] - 2026-05-16

Faithful feedback / reconciliation re-model — the EMIR/SFTR TR
feedback and reconciliation messages are now modelled faithfully to
the real ESMA ISO 20022 schemas, and the synthetic dishonest SFTR
feedback path is removed. Includes one breaking CLI change (the
`opendqi sftr feedback` subcommand no longer exists; `opendqi sftr
tr-audit` is TAR+TSR-only). EMIR feedback, the shared
`FeedbackRecord`/`feedbacks` store and the `opendqi feedback`
workflow are unchanged.

### Removed

- **Synthetic SFTR rejection-feedback path.** SFTR has no
  rejection-feedback message — real `auth.080` is a *reconciliation
  status advice* (handled by `opendqi sftr reconcile` → `SFTR.REC.*`).
  The synthetic `opendqi sftr feedback` command, its
  `auth.080.001.01` parser (`read_sftr_feedback_xml`), the
  `examples/sftr/feedback/` fixture and the four `SFTR.FBK.*` checks
  are **removed** (breaking: the `sftr feedback` subcommand and the
  SFTR "feedback" web-UI operation no longer exist). Consequently
  **`opendqi sftr tr-audit` is now TAR+TSR-only** — its `--feedback`
  argument is gone and the feedback-dependent
  `SFTR.AUD.REJECTED_BUT_OUTSTANDING_IN_TSR` cross-layer check is
  removed for SFTR (it remains EMIR-only; SFTR keeps the two TAR↔TSR
  `SFTR.AUD.*` coherence checks). EMIR feedback (`auth.092`,
  `EMIR.FBK.*`, `EMIR.AUD.*`, `opendqi emir feedback` / `tr-audit`),
  the shared regime-tagged `FeedbackRecord` / `feedbacks` store table
  / `opendqi feedback list/resolve/stale/analytics` workflow, and the
  `SFTR.PSC.*` rejection-profile loop are **unchanged**. This
  completes the Milestone 0.4 faithful feedback/reconciliation
  re-model. See
  [`docs/auth-messages/sftr-auth080.md`](docs/auth-messages/sftr-auth080.md).

### Added

- **Faithful `auth.092` validation-rule list (end-to-end).** EMIR
  rejection feedback (`auth.092`) lists several `DtldVldtnRule` codes
  per rejected transaction; OpenDQI now keeps the **full list**
  (`FeedbackRecord.validation_rule_codes`) instead of only the first.
  The scalar `reason_code` is retained (= the first rule) for
  backward compatibility. Rejection analytics and `rejection_profile.yml`
  now count **each** validation rule (per-rule fan-out), and
  `EMIR/SFTR.FBK.TR_REJECTED_UTI` surface the full list in the issue
  message and as structured `evidence`. `rejections.csv` gains an
  additive `validation_rule_codes` column. Backed by a
  backward-compatible additive SQLite migration
  (`m0002`, `feedbacks.validation_rule_codes_json`); pre-existing
  stores upgrade transparently (old rows read as an empty list).
  Check IDs, the `FeedbackType` enum, the `rejection_profile.yml`
  schema and the `*.PSC.*` loop are unchanged.
- **Real SFTR `auth.080` parser, re-homed into reconciliation.** Real
  `auth.080.001.02` is a *Reconciliation Status Advice* (not rejection
  feedback). A schema-aligned parser is added in `reconciliation.rs`
  and reached via **`opendqi sftr reconcile`** (namespace-dispatched
  alongside the synthetic `auth.083`), projecting onto the existing
  `ReconciliationRecord` (`Mtchd`→PAIRED/RECONCILED;
  `NotMtchd`→PAIRED/UNRECONCILED + the mismatched-criteria field
  names; `NoRcncltnReqrd`→no assertion). `DataSetActn=NOTX` →
  `SFTR.FMT.RCNCLN_NO_RECORDS`. Consequently SFTR has no
  rejection-feedback message: `auth.080` no longer flows through
  `sftr feedback`, and the `SFTR.FBK.*` checks have no real SFTR
  input (`SFTR.REC.UNPAIRED_TRADE` is also unreachable — "unpaired" is
  summary-only in `auth.080`). No model/check/store-schema change; the
  synthetic `auth.083`/`auth.106` paths are untouched. See
  [`docs/auth-messages/sftr-auth080.md`](docs/auth-messages/sftr-auth080.md).
- **EMIR `auth.091` per-transaction reconciliation detail.** The
  `auth.091` parser previously kept only the derived cohort
  pairing/recon **rates**; it now *additionally* projects each
  `TxDtls/RcncltnRpt` onto a `ReconciliationRecord` (UTI from
  `TxId/UnqIdr/UnqTxIdr`; reporting/other counterparty from
  `CtrPtyId`; pairing/recon status **inherited from the enclosing
  cohort** `Pairg`/`Rcncltn`; `mismatched_fields` = the `MtchgCrit`
  criterion names whose `Val1` ≠ `Val2`). `opendqi emir recon-stats`
  (CLI and the local web UI's recon-stats operation — shared core)
  runs the existing `EMIR.REC.*` checks on these and folds their
  issues into `recon_stats_issues.csv`. All three `EMIR.REC.*` are
  reachable from real auth.091. No `--store`/persistence, and no
  canonical-model / check / store-schema change. See
  [`docs/auth-messages/emir-auth091.md`](docs/auth-messages/emir-auth091.md).

### Changed

- Workspace version `0.3.0` → `0.4.0`.

## [0.3.0] - 2026-05-15

Real TR Schema Hardening — the TR feedback/state parsers are now
aligned with the real ESMA ISO 20022 schemas (read locally; the
SWIFT-licensed XSDs are never redistributed). Backwards-compatible:
no canonical-model, check-ID or store-schema change.

### Added

- **Real ESMA schema alignment of the TR feedback/state parsers.**
  Coverage moves `verified (synthetic schema)` →
  **`schema-verified (subset)`** for EMIR `auth.107` (Trade State
  Report), `auth.108` / `auth.109` (Margin Activity / State),
  `auth.091` (Derivatives Trade Reconciliation Statistical Report) and
  `auth.092` (Derivatives Trade Rejection Statistical Report), and SFTR
  `auth.079` (SFT Trade State Report). Each parser now anchors on the
  real message envelope and element paths; each ships a per-message
  coverage note under [`docs/auth-messages/`](docs/auth-messages/)
  documenting the extracted-field map, the ignored branches and the
  honest limits (including checks that are unreachable from the real
  message — e.g. `EMIR.MSR.HAIRCUT_OUT_OF_RANGE`,
  `EMIR.RST.OUTSTANDING_UNPAIRED_HIGH`). `auth.091`'s pairing /
  reconciliation rates are *derived* from the real cohort counts.
- **ZIP/GZIP archive ingestion.** Any scan command that accepts a
  file path now also accepts a `.zip` (its `csv` / `xml` / `parquet`
  members are extracted; member directory components are dropped — no
  zip-slip) or a single-stream `.gz` (e.g. `foo.csv.gz`). Extraction
  is to a per-run temp directory, reclaimed by the OS on reboot (same
  contract as `opendqi desktop`). Resolved at the single
  `discover_emir_inputs` chokepoint, so EMIR and SFTR are covered
  together; the previous "archives are not yet supported" error is
  removed.
- **No-activity report handling.** ISO 20022 `DataSetActn = "NOTX"`
  reports now yield zero records plus a single informational note
  (`EMIR.FMT.{TSR,MAR,MSR,FBK,RST}_NO_RECORDS`,
  `SFTR.FMT.SFTR_TSR_NO_RECORDS`) instead of an error.

### Changed

- **Honest message-naming caveats.** Real `auth.092` is a rejection
  *statistics* report (not a per-UTI feed) and real `auth.080` is an
  SFTR *reconciliation status advice* (not rejection feedback); the
  scalar feedback model is documented as a deliberate lossy projection
  and the SFTR `auth.080` path stays honestly `partial`. A faithful
  feedback / reconciliation re-model (repeating validation-rule codes,
  reconciliation-status, hierarchical detail, store migration) is
  tracked as a separate future milestone.
- Workspace version `0.2.0` → `0.3.0`.

### Infrastructure

- CI gained an MSRV job pinned to Rust **1.87.0** and a non-gating
  `cargo-llvm-cov` coverage workflow.
- Dev/test builds use `debug = "line-tables-only"` (smaller, faster
  links; backtraces keep file:line); a local Cargo tuning directory
  `/.cargo/` is gitignored.

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

[Unreleased]: https://github.com/PauFou/OpenDQI/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/PauFou/OpenDQI/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/PauFou/OpenDQI/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/PauFou/OpenDQI/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/PauFou/OpenDQI/releases/tag/v0.1.0
