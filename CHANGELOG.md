# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **`SortedIssueSink` + k-way-merge engine — unwired (Milestone
  0.22; Increment B of the streaming issue pipeline).** New
  `opendqi_core::{SortedIssueSink, SortedIssues}`: buffers issues
  (applying severity overrides + online `IssueAggregator` tally as
  they arrive), spills `issue_cmp`-sorted JSON-Lines runs to a temp
  dir once a buffer threshold is hit, and `finish()` yields the
  `ScanSummary` plus an iterator that emits every issue in exact
  `issue_cmp` order via a `BinaryHeap` k-way merge — the no-spill
  path being *literally* `finalize_issues` (byte-identical), the
  spill path finalize-equivalent (same multiset, non-decreasing
  under `issue_cmp`). RAII temp-dir cleanup. The 8-field comparator
  is extracted to a single `issue_cmp` shared by `sort_issues` and
  the merge so they cannot drift (output-invariant refactor). No new
  crate (`serde_json` was already a workspace dep). **Dormant** —
  not wired into any command (Increment C flips EMIR `run_scan` and
  measures); zero behaviour / golden / conformance change; 11 new
  exhaustive equivalence/RAII tests.

- **Large-input scale benchmark + end-to-end memory/time harness
  (tooling).** First increment of the performance/scale work
  ("measure before optimize"): the `check_loop` criterion bench now
  covers **1M** records (EMIR + SFTR), a dependency-free streamed
  synthetic ISO-20022 XML generator
  (`opendqi-core/examples/gen_synthetic_xml.rs`) and an opt-in
  `scripts/bench-scale.sh` measure the **whole `opendqi scan`
  pipeline** (parse + checks + write) wall-time and peak RSS; the
  baseline is recorded in [`docs/performance.md`](docs/performance.md).
  Deliberate local release tool — **not** wired into
  `scripts/preflight.sh` or CI (those stay debug). Tooling only: no
  check / model / output / count change; no optimization yet (the
  baseline drives the deferred streaming / incremental-scan work).

- **Phase-boundary RSS attribution for `scan` (tooling).** Opt-in
  `OPENDQI_MEM_TRACE` (surfaced by `scripts/bench-scale.sh
  --mem-trace`) samples current RSS at six `run_scan` boundaries
  (discovery / parse / checks / lifecycle+presub / finalize / report).
  Measured finding (in [`docs/performance.md`](docs/performance.md)):
  the dominant phase **differs by regime** — SFTR 1M peaks at the
  parse+checks steady state (~2.0 GiB), EMIR 1M peaks as a ~2 GiB
  transient inside the finalize→report span (total 3.4 GiB, far above
  any boundary sample). Replaces M0.14's reverted *guess*. Env-gated:
  unset ⇒ byte-identical scan output (golden / XSD-conformance
  unchanged); not in preflight/CI. Measurement only — no optimization.

- **Phase-correlated peak RSS sampler — EMIR culprit localized
  (tooling).** Extends `OPENDQI_MEM_TRACE` with four finer report-span
  markers and a background sampler (default 200 ms,
  `OPENDQI_MEM_TRACE_MS`) that catches a transient freed *between*
  boundary samples and names the phase live at the run maximum.
  Resolved finding (in [`docs/performance.md`](docs/performance.md)):
  **the EMIR 1M peak is `finalize_issues`** — resident jumps
  1471→3819 MiB across it (a *persistent* ~2.35 GiB, scale-dependent:
  absent at 100k), then `write_issues_csv` *frees* ~2.2 GiB; the
  sampler peak (3901 MiB) independently matches `/usr/bin/time`
  (3998 MiB) within ~2 %. SFTR confirms M0.15 (records+issues
  co-residence during checks). Drives the next, optimization
  increment. Env-gated/output-invariant; not in preflight/CI.
  Measurement only — no optimization.

- **Opt-in dhat heap profiler — definitive allocation attribution
  (Milestone 0.18; tooling).** Feature-gated `dhat-heap`
  (`cargo run --release -p opendqi-cli --features dhat-heap`) swaps
  in the dhat global allocator + heap profiler; off by default ⇒
  `dhat` absent from the dependency graph, system allocator
  unchanged, every committed artifact byte-identical (not in
  preflight/CI). Ended the phase-RSS guessing that misled M0.16/0.17:
  the EMIR peak is, by call stack (in
  [`docs/performance.md`](docs/performance.md)), the resident
  `Vec<DqIssue>` (`run_all` collect + `run_scan` extends) **plus
  rayon's parallel-collect intermediate buffers** (~1.5 GB-equiv at
  1M — the elusive transient doubling) **plus** per-issue `format!`
  message strings — **not** a report-write transient. Drives the
  next, evidence-justified optimization. Measurement only — no
  optimization; opt-in, output-invariant.

### Changed

- **`IssueAggregator` — online summary; 18 `build_summary` copies
  de-duplicated (Milestone 0.21).** New
  `opendqi_core::IssueAggregator` computes `ScanSummary`
  (severity/dimension counts, total, `quality_score`) from a
  *stream* of issues without retaining them; `scoring::quality_score`
  now delegates to a shared `quality_score_from_counts` so the score
  is reproducible from counts alone. The 17 CLI + 1 server
  hand-rolled `build_summary` bodies collapse to thin adapters over
  it. **Output byte-identical** (same arithmetic — zero
  golden/conformance diff; preflight green). Foundational seam
  ("Increment A") for the streaming issue pipeline that will replace
  the resident `Vec<DqIssue>` (the only remaining lever after the
  M0.18–0.20 dhat findings); independently valuable as de-dup. No
  behaviour/model/store change.

- **`collect_finalize` is now a single-buffer issue sink
  (Milestone 0.20; refuted memory hypothesis, kept as a refactor).**
  The shared `run_all*` chokepoint replaces the `Vec<Vec<DqIssue>>`
  collect with a `Mutex<Vec<DqIssue>>` fed by `par_iter().for_each`
  — each per-check `Vec` is appended and freed as its check
  finishes, so per-check `Vec`s are no longer all held at once.
  Intended to drop the ≈2× collect transient; the dhat
  evidence-loop **refuted** the headline: total live heap is flat
  (M0.18 752 → M0.19 808 → **M0.20 731 MB** at N=100k) — the
  per-check hold is gone but replaced by the sink's own
  geometric-growth realloc (256 MB single site at 100k). **Fourth**
  consecutive correct-but-headline-flat memory change; the contained
  collect/append lever is **exhausted** — only the out-of-scope
  external-sort / no-retain rearchitecture can move the EMIR peak
  (see [`docs/performance.md`](docs/performance.md)). Kept solely
  for the structural cleanliness: **output byte-identical** (zero
  golden/conformance diff — append order irrelevant,
  `finalize_issues` unchanged; M0.19 unit tests pass as-is),
  preflight green. Not a perf win — stated plainly
  (M0.14/M0.17/M0.19 discipline).

- **`run_all*` consolidated into one `collect_finalize` helper
  (Milestone 0.19; refuted memory hypothesis, kept as a refactor).**
  All 23 duplicated `run_all*` bodies now delegate to a single
  generic helper (−115 lines). It was *intended* to remove a rayon
  parallel-collect doubling (M0.18-attributed) but the dhat
  evidence-loop **refuted** that: collecting `Vec<Vec<DqIssue>>` plus
  a pre-sized destination coexist at ≈2×, *relocating* the doubling
  rather than removing it (total live heap 752 → 808 MB at N=100k;
  EMIR 1M RSS unchanged at ~3.9 GiB). Proven structural conclusion
  (in [`docs/performance.md`](docs/performance.md)): the EMIR peak
  needs the *bounded-memory / issue-streaming* rearchitecture — no
  collect tweak moves it. Kept solely for the de-duplication:
  **output byte-identical** (zero golden/conformance diff — the
  collect is order-preserving, `finalize_issues` unchanged),
  preflight green. Not a perf win — stated plainly (M0.14/M0.17
  discipline).

- **In-place issue sort — eliminates the `finalize_issues`
  stable-sort allocation (Milestone 0.17; honest result).** First
  optimisation of the perf chantier. `sort_issues` now uses
  `sort_unstable_by` (ipnsort, fully in place) instead of the stable
  `sort_by` (driftsort, O(N) scratch of `size_of::<DqIssue>()`/elem),
  with the comparator extended to a **deterministic content total
  order** (`check_id, source_file, record_id, uti, field, value,
  message, evidence`). Measured (in
  [`docs/performance.md`](docs/performance.md)): the EMIR 1M
  *persistent post-finalize footprint drops 3819 → 1530 MiB
  (−2.3 GiB)* — but **total peak RSS is unchanged (~4 GiB)**: the
  binding peak relocated to a previously-masked **report-write
  transient** (next increment's target). Shipped as a correct,
  necessary structural fix — **not** a peak win (it isn't one yet);
  SFTR unaffected. **Behaviour:** `issues.csv` tie-order is now
  content-deterministic (was the parallel-insertion artifact); a
  single golden (`sftr-reconcile.issues.csv`) regenerated — a proven
  pure permutation (identical row set, zero rows added/removed/
  modified, `*.summary.json` byte-unchanged). `EvidenceItem` gains a
  derived `PartialOrd, Ord` (additive, not serialised).

### Removed

## [0.9.0] - 2026-05-17

Post-TR intelligence depth + web-UI parity. EMIR `auth.106` is now
modelled at all three levels — report-level, per-counterparty
(`Wrnngs`) and per-UTI (`Wrnngs/TxDtls`) — with the amount `Ccy`
currency attribute preserved; SFTR `auth.083` gains the
`--tsr`/`--store` trade-state cross-reference (CLI) and its optional
web-UI companion, plus `OthrMstrAgrmtDtls`. Backwards-compatible:
additive checks / records / CLI flags / web-UI companion only — no
existing canonical-model, check-ID, output or store-schema change
(the `auth.106` parser enrichment is output-invisible). Workspace
202 → 213 checks (EMIR 140 → 148, SFTR 62 → 65).

### Added

- **Web UI parity for the SFTR `auth.083` cross-reference.** The
  desktop `missing-collateral` operation now accepts an optional
  `auth.079` TSR companion (the shared `file_tsr` upload): when
  present, the 3 `SFTR.MCR.*` cross-reference checks
  (`COLLATERAL_PRESENT_IN_TSR` / `STILL_MISSING_IN_TSR` /
  `REQUESTED_UTI_NOT_IN_TSR`) run in the web UI, matching the CLI
  `--tsr`. Single-file uploads still run the 2 base checks only. The
  store-backed cross-ref stays CLI-only (the web UI has no history
  store). Server-only change — no model/check/count change; mirrors
  the existing multi-file dispatch (`tr-audit`/`book-reconcile`).
  Docs: [`docs/desktop-web-ui.md`](docs/desktop-web-ui.md),
  [`docs/auth-messages/sftr-auth083.md`](docs/auth-messages/sftr-auth083.md).

- **EMIR `auth.106` amount `Ccy` currency now preserved.** The
  warnings parser captured element text only, so the `Ccy` attribute
  on `Wrnngs/TxDtls` amount leaves (`ValtnAmt`, `NtnlAmt`) was
  dropped. It is now kept alongside the value in the per-UTI
  `raw_fields` via the codebase `text|Ccy=XXX` `encode_value` idiom
  (the same convention as the `auth.030`/`auth.052` catch-all
  parsers). This closes the last documented `auth.106` limitation —
  every `TxDtls` leaf is now preserved (`NtnlQty`/`DerivEvtTmStmp`
  were already kept as text). No model, check, count, or output
  change (`raw_fields` is not in `issues.csv`/`summary.json`);
  attribute-free leaves serialise byte-identically. Docs:
  [`docs/emir-warnings.md`](docs/emir-warnings.md),
  [`docs/auth-messages/emir-auth106.md`](docs/auth-messages/emir-auth106.md).
- **EMIR `auth.106` per-counterparty `Wrnngs` detail.** The
  Data-Quality Warnings parser now also models the per-counterparty
  breakdown: one `WarningsCounterpartyRecord` per `(RefDt, CtrPty
  LEI)`, merging the three `MssngValtn` / `MssngMrgnInf` / `AbnrmlVals`
  `Wrnngs` blocks for that LEI. Drives 5 new
  `EMIR.WRN.CTRPTY_*_HIGH` checks (same rate semantics/thresholds as
  the report-level family, applied per counterparty, LEI named in the
  issue), folded into the same `warnings_issues.csv` (CLI + web UI;
  shared core). EMIR check total 140 → 145, workspace 202 → 207. The
  report-level aggregate and the 5 existing `EMIR.WRN.*` checks are
  unchanged; per-counterparty values never leak into the aggregate
  (integration-asserted). Docs:
  [`docs/emir-warnings.md`](docs/emir-warnings.md),
  [`docs/auth-messages/emir-auth106.md`](docs/auth-messages/emir-auth106.md).
- **EMIR `auth.106` per-UTI `Wrnngs/TxDtls` detail.** The deepest
  warnings level is now modelled: one `WarningsTransactionRecord` per
  transaction the TR explicitly flagged, with `warning_category`
  (`MissingValuation` / `MissingMargin` / `AbnormalValue`),
  `counterparty_lei` (inherited from the enclosing `Wrnngs`), `uti`,
  `other_counterparty`, and the heterogeneous per-category context in
  `raw_fields`. Drives 3 new operational checks (one issue per flagged
  transaction, like `EMIR.REC.*`): `EMIR.WRN.TX_MISSING_VALUATION`
  (Completeness/High), `EMIR.WRN.TX_MISSING_MARGIN` (Completeness/High),
  `EMIR.WRN.TX_ABNORMAL_VALUE` (Accuracy/High), folded into the same
  `warnings_issues.csv` (CLI + web UI; shared core). EMIR check total
  145 → 148, workspace 210 → 213. The report-level + per-counterparty
  records and the 10 existing `EMIR.WRN.*` / `CTRPTY_*` checks are
  unchanged; per-UTI values never leak into the upper levels
  (integration-asserted); the conformance fixture gained valid
  `TxDtls` and still validates against the real ESMA auth.106 XSD.
  All three `auth.106` levels are now modelled. Docs:
  [`docs/emir-warnings.md`](docs/emir-warnings.md),
  [`docs/auth-messages/emir-auth106.md`](docs/auth-messages/emir-auth106.md).
- **SFTR `auth.083` trade-state cross-reference + `OthrMstrAgrmtDtls`.**
  `opendqi sftr missing-collateral` gains `--tsr <auth079>` /
  `--store <db>` (`--tsr` wins): the requested UTIs are matched
  against the firm's SFTR trade state, yielding 3 new `SFTR.MCR.*`
  cross-ref checks — `COLLATERAL_PRESENT_IN_TSR` (Info, likely
  satisfied / TR lag), `STILL_MISSING_IN_TSR` (High, gap confirmed),
  `REQUESTED_UTI_NOT_IN_TSR` (High, SFT absent from TR state). No-UTI
  records are skipped; with neither flag the cross-ref no-ops (output
  byte-identical). auth.083 persists nothing — the cross-ref is
  read-only against the existing `sftr_tr_state_records` history (no
  new table/migration); the web UI keeps the base 2 checks only
  (cross-ref CLI-only, FBK precedent). The previously-dropped
  `MstrAgrmt/OthrMstrAgrmtDtls` free-text is now preserved in
  `raw_fields["MstrAgrmt/OthrMstrAgrmtDtls"]`. SFTR check total
  62 → 65, workspace 207 → 210. Docs:
  [`docs/sftr-missing-collateral.md`](docs/sftr-missing-collateral.md),
  [`docs/auth-messages/sftr-auth083.md`](docs/auth-messages/sftr-auth083.md).

### Changed

### Removed

## [0.8.0] - 2026-05-17

Faithful SFTR `auth.083` Missing Collateral Request — the last real
ESMA message marked "not yet"; the SFTR TR-message surface is now
complete. Backwards-compatible: additive model / checks / CLI / web-UI
op; no existing canonical-model, check-ID or store-schema change.

### Added

- **Milestone 0.8 — faithful SFTR `auth.083` Missing Collateral
  Request.** New `opendqi sftr missing-collateral <auth083.xml>`
  command (and an SFTR-only *Missing Collateral Request* web-UI
  operation) ingesting the real
  `SecuritiesFinancingReportingMissingCollateralRequestV02`
  (`auth.083.001.02_ESMAUG_1.0.0`, schema-verified subset, namespace
  `urn:iso:std:iso:20022:tech:xsd:auth.083.001.02`) — the TR→firm
  request asking the firm to supply the collateral missing for a list
  of SFTs. One `MissingCollateralRecord` per `TxId` (UTI,
  reporting/other counterparty incl. the natural-person
  `OthrCtrPty/Ntrl/Id/Id` branch, master-agreement type/version) and
  2 `SFTR.MCR.*` checks: `MISSING_COLLATERAL_REQUESTED`
  (Completeness/High, one per request) and `MISSING_UTI_ON_REQUEST`
  (Validity/High, when the request omits the UTI). SFTR check total
  60 → 62, workspace 200 → 202; web-UI operations 11 → 12. This was
  the last real ESMA message marked "not yet" — the SFTR TR-message
  surface is now complete. The `auth.083` XSD-conformance case joins
  the local-only gate (`xsd_conformance`, 11/11 with
  `OPENDQI_XSD_DIR` set). Docs:
  [`docs/sftr-missing-collateral.md`](docs/sftr-missing-collateral.md),
  [`docs/auth-messages/sftr-auth083.md`](docs/auth-messages/sftr-auth083.md).

### Changed

- **Golden snapshot harness is now calendar-day-stable.** The
  deterministic-output goldens embed `ctx.today` (maturity-in-past
  messages) and a `today − state_as_of` day-count (margin-state
  staleness), so they previously drifted across midnight. `normalize()`
  now masks `today=YYYY-MM-DD` → `today=<DATE>` and `… is <N> days old
  (threshold <M>)` → `<DAYS>` (the configured `<M>` is kept). Only the
  two affected goldens (`emir-msr`, `sftr-scan`) changed, and only
  those placeholder substitutions — no behavioural output change.

### Removed

## [0.5.0] - 2026-05-16

Reliability hardening — a golden snapshot regression harness, an
adversarial parser-robustness suite, and a parse-path panic-freedom
audit. Backwards-compatible: no canonical-model, check-ID or
store-schema change.

### Added

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

## [0.6.0] - 2026-05-16

Faithful EMIR `auth.106` data-quality warnings, plus removal of the
synthetic `auth.106`/`auth.083` reconciliation path. **Breaking:** the
`opendqi emir reconcile` subcommand no longer exists (EMIR has no
counterparty reconciliation message).

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
  removed below.)

### Removed

- **Synthetic `auth.106`/`auth.083` reconciliation path.** Reading the
  real ESMA XSDs showed both were mislabelled: `auth.106` is a
  data-quality *warnings* report (now modelled — see Added) and
  `auth.083` is a *Missing Collateral Request*; EMIR has no
  counterparty pairing/reconciliation message. The synthetic
  `opendqi emir reconcile` command, the synthetic
  `read_emir_reconciliation_xml` parser path, the `auth.106`/`auth.083`
  synthetic namespace handling and the synthetic
  `examples/{emir,sftr}/reconciliation/auth1{06,}-/auth083-sample`
  fixtures are **removed** (breaking: `opendqi emir reconcile` no
  longer exists). `read_sftr_reconciliation_xml` now accepts **only**
  the real `auth.080.001.02`. The `EMIR.REC.*` / `SFTR.REC.*` checks
  are **unchanged and kept** — `EMIR.REC.*` is fed by the real
  `auth.091` per-transaction detail (`opendqi emir recon-stats`,
  Milestone 0.4) and `SFTR.REC.*` by the real `auth.080`
  (`opendqi sftr reconcile`); no check ID, the `ReconciliationRecord`
  model or the `reconciliations` store changed. Completes the
  Milestone 0.6 faithful `auth.106`/`auth.083` re-model. See
  [`docs/auth-messages.md`](docs/auth-messages.md).

## [0.7.0] - 2026-05-16

XSD-conformance reliability gate — every schema-verified message now
has a fully-XSD-valid conformance fixture validated against the real
ESMA XSD. Backwards-compatible: no canonical-model, check-ID or
store-schema change.

### Added

- **XSD-conformance reliability gate.** Each schema-verified message
  now ships a **fully XSD-valid** conformance fixture
  (`examples/emir/conformance/auth0{30,91,92}-valid.xml`,
  `auth1{06,07,08,09}-valid.xml`,
  `examples/sftr/conformance/auth0{52,79,80}-valid.xml` — all 10
  schema-verified messages). The new
  `crates/opendqi-xml/tests/xsd_conformance.rs` strictly validates
  each against the **real ESMA XSD** via `xmllint` (reusing
  `ExternalXmllintValidator`) **and** round-trips it through the
  parser (records produced, no format issues) — closing the last
  reliability caveat that parsers had only seen schema-shaped
  *subset* fixtures. The gate is **developer/preflight-local and
  self-skips in public CI**: it activates only when `xmllint` is
  present and `OPENDQI_XSD_DIR` points at locally-extracted real ESMA
  XSDs (SWIFT-licensed, gitignored, never committed). The lean
  parser/golden/robustness fixtures are unchanged (still documented
  schema-shaped subsets). See
  [`docs/xsd-validation.md`](docs/xsd-validation.md).

### Changed

- Workspace version `0.4.0` → `0.7.0`.

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

[Unreleased]: https://github.com/PauFou/OpenDQI/compare/v0.9.0...HEAD
[0.9.0]: https://github.com/PauFou/OpenDQI/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/PauFou/OpenDQI/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/PauFou/OpenDQI/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/PauFou/OpenDQI/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/PauFou/OpenDQI/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/PauFou/OpenDQI/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/PauFou/OpenDQI/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/PauFou/OpenDQI/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/PauFou/OpenDQI/releases/tag/v0.1.0
