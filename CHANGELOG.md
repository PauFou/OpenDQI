# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

### Changed

### Removed

## [0.12.3] - 2026-05-21

Adoption polish + race-condition fix. Now that `pip install
opendqi` works (since v0.12.2), this release tightens the
onboarding path: a 5-line `examples/python/quickstart.py` for
the copy-paste smoke test, a README install section restructured
with Python first, and a structural fix that prevents the
`Release` vs `Python Release` race condition we hit on v0.12.2
(which required a manual `gh release upload` workaround).

**No new Python API. No Rust core change.** The 6 entry points
(`opendqi.{emir,sftr}.{scan_parquet, scan_table, parse_xml}`)
are unchanged. The 216 checks remain 216 (151 EMIR + 65 SFTR).
The v1.0 stable Arrow contract for `result.issues` (locked in
v0.12.0 P3) is unchanged. 19/19 goldens byte-identical. 762/0
workspace tests. 30/30 pytest.

### Added

- **`examples/python/quickstart.py`** — the 5-line "does it
  work?" check. 3 lines of opendqi + 2 of print. Distinct from
  the 3 progressively-realistic numbered scripts
  (`01_scan_parquet.py`, `02_parse_xml_then_scan.py`,
  `03_custom_mapping.py`) which stay as deeper examples. Path
  relative to repo root (unlike the numbered scripts which do
  their own path discovery) — designed to be the most natural
  thing after a fresh `git clone`. Smoke-tested locally:
  prints summary `{records_processed: 20, issues_total: 197,
  quality_score: 27.85}` + first 5 rows of the issues
  `to_pandas().head()`, matching the existing CLI golden
  `emir-tr-activity` numbers exactly.

### Changed

- **`README.md` restructured for the `pip install` era.** Three
  edits:
  - One-liner (line 6) replaced — `OpenDQI turns... actionable
    data quality intelligence` → `Turn EMIR/SFTR Trade Repository
    files into deterministic data quality reports and Arrow
    tables — locally, reproducibly, from your existing data
    stack.` More punchy, mentions Arrow, removes the redundant
    `OpenDQI` prefix (the H1 already says it).
  - Lead paragraph (line 8) gains a sentence on the 3 channels
    explicitly: `Use it from your terminal (CLI), your browser
    (local web UI on http://127.0.0.1:7878), or your Python /
    PyArrow notebook.`
  - Install section entirely refondue — order is now **Python
    (`pip install opendqi`) > CLI installer (`curl -sSL ...
    installer.sh | sh`) > Rust source (`cargo install --git
    --tag v0.12.2`)**, each with a one-line "recommended for…"
    note. Followed by a 5-line code block showing the canonical
    Python smoke test (matches `examples/python/quickstart.py`
    exactly). The Python install block now also cross-references
    `quickstart.py`, the 3 numbered scripts, the Jupyter
    notebook, `docs/python.md`, and `docs/python-roadmap.md` in
    one paragraph. Bumped all install-command tag references
    from `v0.11.0` (4 releases stale) to `v0.12.2`.
- **`examples/python/README.md`**: `quickstart.py` promoted to
  the top of the script table ("Start here" / "Your first run
  30-second smoke test"). The 3 numbered scripts move under
  "The three deeper patterns".

### Removed

- **`release` job from `.github/workflows/python-release.yml`.**
  This job used `softprops/action-gh-release@v2` to attach
  wheels to the GitHub Release; that action CREATES the release
  if missing, racing with cargo-dist's `host` job. When
  python-release became faster than cargo-dist (which happened
  on v0.12.2 once `macos-13` was dropped from the wheel matrix),
  cargo-dist's `host` failed with `release already exists` and
  required a manual `gh release upload` of all 13 cargo-dist
  artefacts to recover. The two workflows are now structurally
  orthogonal: `release.yml` (cargo-dist) always owns the GitHub
  Release page; `python-release.yml` always owns PyPI. No race
  condition possible. Users who want a wheel URL fallback can
  `pip download opendqi==X.Y.Z --no-deps --dest ./wheels/` or
  grab the workflow artefacts from the Actions UI (90-day
  retention). The workflow-level `permissions: contents: write`
  is also gone (only `release` needed it; `publish` declares
  `id-token: write` on its own scope). Workflow header comment
  refondu (3 jobs → 2 jobs + v0.12.3 rationale paragraph
  documenting the race condition for future maintainers).

## [0.12.2] - 2026-05-20

CI hotfix — drop `x86_64-apple-darwin` (macOS Intel) from the
Python wheel matrix. v0.12.0 and v0.12.1 `Python Release`
workflows both hung indefinitely on the `macos-13` runner
(the only GitHub-hosted free-tier path to Intel macOS) — the
runner pool is severely over-subscribed since the deprecation
announcement late 2025, and our jobs queued 1h+ without ever
starting. The other 3 wheels (Linux x86_64 + ARM64 + macOS
ARM64 / Apple Silicon) build in ~5min each, but the
`Python Release` workflow as a whole can't proceed to the
`release` / `publish` jobs until the matrix completes.

This patch drops the Intel-macOS row entirely so the workflow
ships in ~10 min end-to-end, including the PyPI publish.

**No Rust code change. No Python API change.** The 216 checks
remain 216. The v1.0 stable Arrow contract for `result.issues`
is unchanged. 19/19 goldens byte-identical, 762/0 workspace
tests, 30/30 pytest.

Intel-Mac users (free-tier hardware is rare in 2026 — Apple
Silicon has been the default since late 2020) can install via:
- `cargo install --git https://github.com/PauFou/OpenDQI --tag
  v0.12.2 opendqi-cli` for the CLI binary (the cargo-dist
  `release.yml` workflow still ships an Intel-macOS binary —
  it uses a different runner path that isn't oversubscribed)
- the Linux x86_64 wheel under Rosetta 2 for the Python
  bindings (`pip install opendqi --platform manylinux2014_x86_64
  --only-binary opendqi --target ./vendor` workaround if
  needed, though plain `pip install opendqi` will still pick
  the Linux wheel if `pyarrow` is already installed for the
  right ABI).

### Changed

- **`.github/workflows/python-release.yml` matrix**: 4 targets
  → 3 targets. Dropped row: `{ os: macos-13, target:
  x86_64-apple-darwin, manylinux: "off" }`. Inline comment on
  the matrix documents the rationale + the install workarounds
  for Intel-Mac users. Header comment updated (4 → 3 targets).
- **`docs/python.md` Install** section: "Four wheels per
  release" → "Three wheels per release", with a paragraph
  explaining the Intel-macOS drop and the two install
  workarounds.
- **`examples/python/README.md`**: same install-block update +
  reference to v0.12.2 as the first PyPI-published version.

### Removed

- **macOS x86_64 (Intel) abi3 wheel** from
  `python-release.yml`. Re-add when GitHub provides a non-
  deprecated x86_64 macOS runner with a usable free-tier
  queue, or when we move to a paid `macos-13-large` plan.

## [0.12.1] - 2026-05-20

Python packaging polish + PyPI publish + adoption kit.

v0.12.0 shipped the Python bindings as 4 wheels on the GitHub
Release page, but installation was `pip install <wheel URL>`
— friction high. v0.12.1 closes that gap : **`pip install
opendqi` now works** (via OIDC trusted publishing to PyPI), and
the repo gains 3 runnable quickstart scripts + a Jupyter
notebook + a quickstart-facing `docs/python.md`. Pure adoption,
zero new Python API surface, zero modification of the Rust core
beyond version bumps.

Backwards-compatible. **Zero code change beyond version bumps**
in the 3 version-bearing files (`Cargo.toml`,
`crates/opendqi-py/Cargo.toml`, `crates/opendqi-py/pyproject.toml`)
and the auto-refreshed lockfiles. **216 checks remain 216**.
19/19 goldens byte-identical. 762/0 workspace tests. 30/30
pytest. The v1.0 stable Arrow contract for `result.issues`
(locked in v0.12.0 P3) is unchanged.

### Added

- **`pip install opendqi` via OIDC trusted publishing.**
  `.github/workflows/python-release.yml` gains a third job
  `publish` that runs in parallel with the existing `release`
  (GitHub Release attach) after the `build` matrix. Uses
  `pypa/gh-action-pypi-publish@release/v1` with
  `permissions: id-token: write` (the OIDC scope PyPI's
  trusted-publisher endpoint requires) — **no `PYPI_API_TOKEN`
  GitHub secret to create / rotate / leak**. Configured via
  `environment.name: pypi`, `environment.url:
  https://pypi.org/p/opendqi`. The publish is idempotent
  (`skip-existing: true`) so a retag after a CI hiccup is safe.
  PyPI-side precondition (one-shot, browser): configure a
  pending publisher for `opendqi` pointing at
  `PauFou/OpenDQI/python-release.yml` + environment `pypi`.

- **`examples/python/` — 3 runnable quickstart scripts + Jupyter
  notebook.** Each script is autonomous, ~30–80 lines, with
  path discovery via `Path(__file__).parents[2]` so it works
  from any CWD :
  - `01_scan_parquet.py` — single-call scan on a normalized
    Parquet (the simplest entry point; generates the Parquet
    on first run via the CLI binary)
  - `02_parse_xml_then_scan.py` — `parse_xml → scan_table`
    chain, no Parquet roundtrip; for in-memory data platforms
  - `03_custom_mapping.py` — `scan_table` with a user-named
    Arrow table + custom column mapping; the realistic
    data-warehouse path
  - `quickstart.ipynb` — same 3 patterns committed
    **executed-with-outputs** for GitHub rendering (built via
    the `_build_notebook.py` helper).
  - `README.md` — local install / run instructions +
    when-to-use-which table.

- **`docs/python.md` — quickstart-facing Python doc (~290
  lines).** Distinct from the existing `docs/python-roadmap.md`
  (architecture spec — kept for historical reference). Covers:
  status header (stable v1.0 Arrow contract vs evolving API) ;
  install ; the 3 patterns ; the 6-function public API surface ;
  the 3 output shapes (`summary` dict, `issues` Arrow Table,
  `normalized` Arrow Table) ; integration patterns for **DuckDB
  / Polars / pandas / Spark** (the Spark section explicitly
  documents the Arrow / Parquet handoff pattern — a dedicated
  `opendqi.spark` namespace with a native `mapInPandas` UDF is
  deferred to v0.13) ; a candid status & limitations list ; a
  pointer to the architecture spec.

### Changed

- **`README.md` Python install block.** Replaces
  `pip install <wheel URL from the v0.12.0 GitHub Release page>`
  with the actual `pip install opendqi`. Adds 4 cross-references
  in the same block (`docs/python.md`, `examples/python/`, the
  notebook, `docs/python-roadmap.md`). The "Documentation →
  Get started" bucket gains `docs/python.md` + `examples/python/`
  alongside the existing CLI quickstart kit (`docs/use-cases.md`
  + `examples/quickstart-emir/` + `scripts/demo.sh`). The
  "What's next" pointer reframes the Python roadmap doc as
  "v0.12 implemented, v0.13+ deferred".

### Removed

## [0.12.0] - 2026-05-20

Python / Arrow bindings preview — OpenDQI becomes embeddable.

The v0.11.0 adoption pack made the product approachable from the
README; v0.12.0 makes it **embeddable** from a Python process.
A new optional Rust crate `crates/opendqi-py/` (PyO3 + maturin)
ships `import opendqi` with three entry points per regime
(`scan_parquet`, `scan_table`, `parse_xml`), returning the
familiar `summary` dict, a `pyarrow.Table` of issues against the
**v1.0 stable 11-column schema**, and an optional canonical-model
`pyarrow.Table` (`normalize=True`). The engine itself is not
reimplemented — every parser, every check, the streaming sink
and the canonical record model are reused as-is from the
existing Rust crates.

The bindings are an additive parallel surface: **zero Rust
business logic was modified** (the only edits beyond `opendqi-py`
itself are 4 visibility bumps on `parquet_out` helpers, the
version bumps, and the new `python-release.yml` CI). Workspace
**216 checks remain 216** (151 EMIR + 65 SFTR), **762/0 workspace
tests**, **19/19 goldens byte-identical** with `UPDATE_GOLDEN`
unset. The Arrow schema for issues matches the existing
`issues.csv` byte-for-byte (verified by `test_issues_schema.py
::test_arrow_schema_matches_csv_golden`) — that's the v1.0
contract.

### Added

- **`crates/opendqi-py/` — new Python / Arrow bindings crate
  (P1 → P5 of the chantier described in
  [`docs/python-roadmap.md`](docs/python-roadmap.md)).**
  PyO3 0.22 + maturin 1.x + `arrow = "53"` (with the `pyarrow`
  feature — the dedicated `arrow-pyarrow` crate only exists from
  55.x), pinned to match the workspace `arrow-array = "53"`.
  abi3-py39 means one wheel per target covers Python 3.9+
  unchanged (forward-compatible to 3.14 — verified locally on
  3.14.5 via `PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1`). The crate
  is deliberately **NOT** in `[workspace] members` of the root
  `Cargo.toml` (uses an empty `[workspace]` opt-out at the top
  of its own manifest) — `cargo test --workspace` never touches
  it, so the existing Rust CI matrix (Ubuntu + macOS, no Python
  venv) is unaffected. Maturin compiles `opendqi-py` independently
  via its own `Cargo.lock`.

- **`opendqi.{emir,sftr}.scan_parquet(path, *, normalize=False)
  -> PyScanResult`** — reads the canonical EMIR/SFTR Parquet via
  `opendqi_io::read_emir_parquet` / `read_sftr_parquet`, runs the
  standard single-batch DQ check suite (`default_checks()` for
  EMIR, `default_sftr_checks()` for SFTR) through the same
  `SortedIssueSink` streaming pipeline the CLI uses (M0.22+,
  `STREAM_SPILL_MAX_ISSUES = 65_536` ⇒ every shipped fixture
  stays in the no-spill path), and returns a `PyScanResult`
  exposing `summary` (dict, 9 fields mirroring `summary.json`),
  `issues` (pyarrow.Table, v1.0 schema), and `normalized`
  (optional pyarrow.Table, canonical model — populated when
  `normalize=True`).

- **`opendqi.{emir,sftr}.scan_table(arrow_tbl, mapping, *,
  normalize=False) -> PyScanResult`** — Arrow-in surface: accepts
  a `pyarrow.Table` or `pyarrow.RecordBatch` and a mapping dict
  of `canonical_field_name → user_column_name` (same direction as
  the existing CSV mapping pattern in
  `crates/opendqi-io/src/csv_in.rs:35-45`). Strict type
  contract: mapped columns MUST have the canonical Arrow type
  (`Utf8`, `Decimal128(38,10)`, `Date32`, `Timestamp(μs,UTC)`,
  `Boolean`) — users with string-only tables cast in Python
  first (`pa.compute.cast(col, pa.date32())`). Unmapped
  canonical fields are emitted as `None` on every record;
  downstream `EMIR.COMP.*` / `SFTR.COMP.*` checks surface the
  missingness naturally. A mapping that points at a non-existent
  user column raises a loud, actionable error (no silent
  all-None records).

- **`opendqi.{emir,sftr}.parse_xml(path) -> pyarrow.Table`** —
  parses any EMIR firm-submission XML (`auth.030.001.03` or
  `.04`) or SFTR (`auth.052.001.02`) into the canonical Arrow
  Table — same schema `opendqi {emir,sftr} normalize` produces.
  Enables the zero-Parquet `parse_xml → scan_table` pipeline.

- **`opendqi.PyScanResult` — the result object exposed to
  Python.** `#[pyclass(frozen)]`, exposes `.summary` (dict),
  `.issues` (pyarrow.Table | None), `.normalized` (pyarrow.Table
  | None), and an informative `__repr__`. The pyarrow Table is
  always single-chunk (constructed via `pa.Table.from_batches([
  recordbatch])`); users who need multi-batch should call
  `.combine_chunks().to_batches()`.

- **`opendqi.<regime>.scan_*` `normalize=True` keyword** —
  populates `result.normalized` with the canonical EMIR/SFTR
  Arrow Table (the same `RecordBatch` the Parquet writer
  produces, post-P0 exposed primitives). Default `False` keeps
  the result zero-overhead for callers who only want issues.

- **v1.0 stable Arrow schema for `DqIssue` exports** — 11
  columns (`check_id`, `regime`, `severity`, `dimension`,
  `record_id`, `uti`, `field`, `value`, `message`,
  `source_file`, `evidence_json`), all `Utf8`, nullability
  mirroring the `Option<T>` shape on the Rust struct. Column
  names + order match
  `crates/opendqi-report/src/csv_out.rs::write_issues_csv`
  byte-for-byte — verified by a parity test that loads the
  existing CLI golden (`emir-scan-csv.issues.csv`) via
  `pyarrow.csv.read_csv` and asserts schema equality on
  column-name list. **Any future change to this schema is a
  BREAKING change.**

- **`.github/workflows/python-release.yml` — wheel build + GitHub
  Release upload via `PyO3/maturin-action@v1`.** Triggered by the
  same `v*` tag push as `release.yml` (cargo-dist); wheels are
  added as additional assets to the GitHub Release the
  cargo-dist workflow creates — same tag, same release. 4
  targets matching cargo-dist exactly: Linux x86_64
  (manylinux2014) + Linux ARM64 (manylinux_2_28) + macOS
  x86_64 (macos-13 runner) + macOS ARM64 (macos-14 runner).
  abi3-py39 ⇒ 1 wheel per target covers Python 3.9+
  unchanged. **No `maturin publish` to PyPI** in this workflow
  — gated on a separate explicit user-ask.

### Changed

- **4 helpers in `crates/opendqi-io/src/parquet_out.rs`
  promoted to `pub`** (`fn` → `pub fn`): `emir_schema`,
  `build_emir_batch`, `sftr_schema`, `build_sftr_batch`. Their
  bodies are unchanged — they are the single source of truth for
  the EMIR/SFTR Arrow projection (Decimal128(38,10) / Date32 /
  Timestamp(μs,UTC) / Utf8) and the bindings now reuse them
  directly to construct the `result.normalized` Arrow Table and
  the `parse_xml` output. The bump is additive (none of the
  prior callers change); re-exported from
  `crates/opendqi-io/src/lib.rs` alongside the existing
  `write_emir_parquet` / `write_sftr_parquet`. Zero golden
  diff, zero behavioural change.

- **`README.md` Install section** — adds a 4-line **Python**
  block alongside the existing CLI install line: `pip install
  <wheel URL>` + `import opendqi; result = opendqi.emir.scan_
  parquet("tsr.parquet")` + a sentence pointing at
  `docs/python-roadmap.md` for the full architecture spec.
  Same engine, same checks, embedded in your Python pipeline.

- **`docs/python-roadmap.md` status header** — bumped from
  "design" to "**IMPLEMENTED in v0.12.0**" with the chain of
  P0-P5 commit references for the historical record. The body
  of the spec is preserved as-is (it became the authoritative
  architecture description).

### Removed

## [0.11.0] - 2026-05-20

Adoption pack + Python/Arrow bindings architecture spec. The
v0.10.0 release shipped a technically mature engine (216 checks,
12 ISO 20022 messages, streaming pipeline with a measured
~32 % EMIR-1M peak RSS reduction, 762 tests, 19/19 goldens
byte-identical). **The risk now is integration, not technical
maturity.** A team running on Databricks / Airflow / a local
DuckDB notebook should be able to (a) understand what OpenDQI
does in 3 minutes, (b) try it in 30 seconds, (c) plan an
embedded integration path. This release closes (a)/(b)/(c).

**Backwards-compatible.** **Zero Rust business logic changed.**
216 checks remain 216 (151 EMIR + 65 SFTR). The 19/19 golden
artefacts are byte-identical. The CLI surface, the canonical
record model, the SQLite store schema, the HTML report
template, and the Parquet output schema are all unchanged. The
only code modification is the workspace version bump and two
cargo-dist additions (`[profile.dist]` in `Cargo.toml` and
`[package.metadata.dist]` in `crates/opendqi-cli/Cargo.toml`).
A new (off-by-default) tag-triggered release workflow is
added — it never runs on main pushes or PRs.

### Added

- **`docs/use-cases.md` — 3 operator-facing scenarios.**
  Translates the abstract 3-layer model (TAR / TSR / Rejection)
  from `docs/positioning.md` into 3 concrete end-to-end
  scenarios with question / input / command / what-you-get /
  what-you-don't-get : **TR state health**
  (`emir tr-state-scan auth107.xml` → 8 records, 16 issues,
  score 86.6 on the shipped fixture); **rejection intelligence**
  (`emir feedback` + `feedback analytics` →
  `rejection_profile.yml` → next `scan --rejection-profile`,
  the post-TR ↔ pre-TR loop — 2 records, 2 critical, score
  75.0); **combined audit** (`emir tr-audit` 3 files + 3
  cross-layer `EMIR.AUD.*` — 20 records, 251 issues, score
  4.4). Every count is byte-pinned by the golden suite
  (`crates/opendqi-cli/tests/golden/emir-{tr-state,feedback,tr-
  audit}.summary.json`), so the document is self-verifying.

- **`examples/quickstart-emir/` — self-contained EMIR
  quickstart kit.** Bundles the 3 fixture files needed for the
  3 primary workflows in a single directory, with a local
  README documenting commands + counts + cross-references.
  Byte-identical copies of the existing
  `examples/emir/{tr_state,tr_activity,feedback}/*sample.xml`
  fixtures (`diff -s` verified) — the originals remain
  referenced by the golden harness.

- **`scripts/demo.sh` — 30-second walkthrough.** POSIX bash
  (macOS bash 3.2 + Linux compatible, `set -u` safe). Builds
  `opendqi` in debug `--jobs 4` only when needed (CLAUDE.md
  build hygiene — never release in the dev loop). Runs the 3
  quickstart-emir workflows, drops 3 `report.html` under
  `/tmp/opendqi-demo/`, opens the consolidated audit report
  via `open` (macOS) / `xdg-open` (Linux), echoes the path
  otherwise. Smoke-tested locally : ~0.25 s wall when the
  binary is already built, full ~10 s on first run.

- **`docs/python-roadmap.md` — executable architecture spec
  for v0.12 Python/Arrow bindings.** ~290 lines, concrete
  enough to be followed verbatim during v0.12 implementation.
  Sections : *Why* (DuckDB analogy — the risk now is
  integration) ; *Scope strict v0.12* (`opendqi.{emir,sftr}.
  scan_parquet` / `scan_table(arrow_tbl, mapping)` /
  `parse_xml` ; outputs `summary: dict`, `issues:
  pyarrow.Table`, `normalized: pyarrow.Table | None`) ;
  *Non-goals* (no native Spark UDFs, no magic DataFrame for
  every lib, no multi-regime, no Python-side web UI, no SaaS) ;
  *Module layout* (new `crates/opendqi-py/` workspace crate,
  independent of the existing 7) ; *Reuse table* showing every
  primitive needed already exists in the Rust core (parsers,
  `default_checks()`, `IssueAggregator`, `SortedIssueSink`,
  Parquet read/write, `EmirRecord` → `RecordBatch` in
  `parquet_out.rs::build_emir_batch`) ; *Arrow schema mapping*
  field-by-field for `EmirRecord` (54 cols), `SftrRecord` (31
  cols) ; **`DqIssue → Arrow` 11-col schema as the v1.0
  stable contract** ; *Build (maturin abi3, 4 targets matching
  the cargo-dist release workflow)* ; *Deps freeze
  (`arrow-pyarrow=53` MUST match workspace `arrow-array=53`)* ;
  *v0.12 milestone breakdown (5 increments P1-P5)* ; *Out of
  scope, deferred to v0.13+*. The v0.12 implementation begins
  on a dedicated feature branch only on explicit user request.

- **`.github/workflows/release.yml` — cargo-dist 4-target
  GitHub Release workflow.** Generated by cargo-dist 0.31.0
  (`dist init --yes --hosting github --installer shell` +
  `--target` × 4). Builds `opendqi` on **Linux x86_64 + ARM64
  and macOS x86_64 + ARM64** (matching the existing CI matrix
  Ubuntu + macOS — no Windows binary), produces per-target
  `.tar.xz` archives with sha256 + a curl-installable
  `installer.sh` + a workspace `sha256.sum`, and uploads them
  to a GitHub Release. **Trigger: `push.tags: [v*]` only** —
  never runs on main pushes, never on PRs (zero charge on the
  dev loop). Configuration lives in the new `dist-workspace.
  toml` (modern format) ; `Cargo.toml` gained `[profile.dist]`
  (inherits release, lto=thin) ; `crates/opendqi-cli/Cargo.
  toml` gained `[package.metadata.dist] dist = true` (the
  `publish = false` would otherwise hide the binary from
  cargo-dist — it ships via GitHub Releases, not crates.io).
  Verified via `dist plan --tag v0.10.0` dry-run.

### Changed

- **`README.md` — refactored around 3 workflows (181 → 122
  lines).** Top of the file now shows **the 3 things OpenDQI
  does** in 3 copy-pasteable code blocks above the fold, each
  with a one-sentence "what you get". A `30-second demo`
  pointer to `scripts/demo.sh` follows. The dense 5-paragraph
  Features list collapsed to a compact 3-row Coverage table
  that **sums correctly** to 216 (133 single-batch + 83
  TR-layer cross-message = 151 EMIR + 65 SFTR). A flat
  4-bucket Documentation list organises the 36 `docs/` pages
  (Get started / Per-workflow / Engineering / Reliability /
  What's next). The Status section now points at the v0.10.0
  D5 honest measurement (~32 % EMIR-1M peak RSS reduction) +
  MSRV 1.87.0 + CI in one paragraph. The obsolete Phase 0-9
  list became a forward-looking 4-row roadmap table (v0.10
  done → v0.11 this release → v0.12 Python preview → v1.0
  stable contract). Preserved unchanged : 2 CI/security
  badges, Shell completions / Contributing / Security /
  Disclaimer / License blocks.

### Removed

## [0.10.0] - 2026-05-20

Streaming-issue pipeline end-to-end + EMIR Article-11 collateral
cross-reference + compression-event quality. The chantier opened by
the M0.13–M0.18 measurement work closes here with the **first real
EMIR-1M peak reduction** of the whole release line (~3.9 → ~2.7 GiB,
~32 %), and the EMIR collateral obligation gets its first
cross-message check pack via a new `emir collateral-audit` subcommand.

Backwards-compatible: every existing command keeps its CLI surface,
output byte-layout, golden artefacts and store schema. Added one new
subcommand (`emir collateral-audit`), one new EMIR check family
(`EMIR.COL.*`, 2 checks), and one new EMIR risk-mitigation check
(`EMIR.RMT.COMPRESSION_EVENT_INCOMPLETE`). Workspace 213 → **216**
checks (EMIR 148 → **151**, SFTR **65**).

### Added

- **EMIR collateral audit — `auth.107` (TSR) ↔ `auth.109` (MSR)
  cross-reference.** New cross-message subcommand
  `opendqi emir collateral-audit --tsr <auth107> --msr <auth109>
  [--config] [--out] [--email-config]` that joins outstanding TSR
  derivatives to MSR margin state by UTI and emits two new
  `EMIR.COL.*` checks (a new cross-message family, cousin of
  `EMIR.AUD.*`): `EMIR.COL.MISSING` (Completeness/High) when no MSR
  row is joinable OR every IM/VM amount (posted + collected, current)
  is absent-or-zero across all matches; `EMIR.COL.STALE`
  (Timeliness/Warning) when the linked non-zero MSR snapshot is older
  than `emir_rmt.collateral_max_age_days` (default 1) vs the TSR
  `state_as_of` (fallback `now`). Closes the missing Article-11
  *collateral* cell in the obligation × {missing, timely} matrix
  (confirmation and valuation were already covered). Free function
  `opendqi_core::dq::compute_collateral_emir_issues(tsr, msr,
  thresholds, now)` (mirror of `compute_tr_audit_emir_issues`).
  Outputs: `summary.json`, `collateral_audit_issues.csv`,
  `collateral_audit_report.html`. Honest scoping caveat (documented):
  `TrStateRecord` carries no clearing-status flag in the ESMA usage
  guideline OpenDQI consumes, so the check applies to every
  outstanding TSR derivative — it is a *data-quality signal*, not a
  verdict of Article-11 non-cleared non-compliance. Fixtures:
  `examples/emir/collateral_audit/{tsr,msr}.xml` covering all 4
  branches (no-link, all-zero, fresh-OK, stale). Docs:
  [`docs/collateral-audit.md`](docs/collateral-audit.md),
  [`docs/emir-risk-mitigation.md`](docs/emir-risk-mitigation.md).

- **EMIR risk-mitigation: `EMIR.RMT.COMPRESSION_EVENT_INCOMPLETE`.**
  New single-batch check (Completeness/Warning, +1 → 11 in the
  `EMIR.RMT.*` family) firing on uncleared records where `event_type`
  ∈ {`COMP`, `NOVA`} and `prior_uti` and/or
  `collateral_portfolio_code` are absent/blank — one issue per
  missing field. Surfaces Article-11(1)(c) portfolio-compression
  reporting gaps that would otherwise make the activity unanalysable.
  Honest framing: a DQ signal on *reported* compression events, not
  an inference of compliance with the obligation to *perform*
  compression (which needs cross-batch portfolio cadence — deferred).
  Fixture row R013 in `examples/emir/risk_mitigation/rmt_sample.csv`.
  Docs: [`docs/emir-risk-mitigation.md`](docs/emir-risk-mitigation.md).

- **`SortedIssueSink` + k-way-merge engine (Milestone 0.22;
  Increment B of the streaming issue pipeline).** New
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
  crate (`serde_json` was already a workspace dep). 11 new
  exhaustive equivalence/RAII tests. *Now wired by M0.23–0.27 below.*

- **Generic `stream_checks_into` helper (Milestone 0.24; Increment
  D₁).** New `opendqi_core::dq::stream_checks_into<C: ?Sized + Sync>(
  checks, sink, run)` in `dq/mod.rs`: the closure captures
  records / prior-history / `CheckContext`, so the helper serves
  every check family (EMIR single-batch, EMIR lifecycle, SFTR
  single-batch, SFTR lifecycle, every TR-message family) without
  per-regime variants. `stream_emir_checks_into` becomes a 1-line
  delegate (byte-identical to M0.23). Two equivalence tests
  (`stream_checks_into_equals_finalize_issues`,
  `stream_checks_into_empty_checks_is_noop`).

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

- **EMIR-1M peak RSS reduced ~3.9 GiB → ~2.7 GiB (~32 %), wall-time
  ~80 s → ~50 s — the first real win of the M0.13→M0.27 chantier
  (Milestones 0.21–0.27, D5).** End-to-end roll-out of the streaming
  issue pipeline: every scan path on CLI and the local web UI now
  pushes issues into a `SortedIssueSink` instead of accumulating a
  resident `Vec<DqIssue>` for the whole batch, then `finish()` yields
  the `ScanSummary` plus a sorted iterator that
  `write_issues_csv_from_iter` consumes lazily. The reduction is
  honest and measured on three independent views (in
  [`docs/performance.md`](docs/performance.md)): `OPENDQI_MEM_TRACE`
  phase trace (the 3819→2369 MiB finalize jump is **gone**;
  post-`write_issues_csv` resident 1585→**8 MiB**), `dhat-heap` live
  heap @100k EMIR scan **752→489 MB** (−35 %; `run_all` collect
  160 MB gone, rayon parallel-collect intermediates ~152 MB gone,
  sink's own buffer only 13 MB), and `/usr/bin/time -l`
  corroborates (OS-max 2722 / sampler 2679 MiB across two 1M runs ⇒
  honest range 2.68–3.24 GiB / ~18–32 %, conservative claim ~32 %
  midline). SFTR gain modest (~9 %, 1M ~2.1→1.95 GiB), exactly as
  predicted — SFTR was never issue-`Vec`-bound. New floor: the
  resident `Vec<EmirRecord>` from parse + per-issue `format!`
  message strings @1M + the bounded sink buffer; future levers are
  out of scope.

  - **M0.23 — EMIR `run_scan` flipped to the sink.** First
    consumer; `STREAM_SPILL_MAX_ISSUES = 65_536` keeps every
    committed golden in the no-spill path (no-spill =
    `finalize_issues`, M0.22-locked ⇒ byte-identical issues.csv /
    summary.json). Bounded top-20 issue selection via `TopIssues`
    heap (online, no full re-scan).
  - **M0.25 — 9 EMIR CLI commands migrated.**
    `run_{feedback, recon_stats, warnings, tr_state_scan, mar_scan,
    msr_scan, tr_activity_scan, tr_audit, book_reconcile}` all
    routed through the sink; the dead `build_summary` /
    `build_feedback_summary` helpers + the unused
    `run_all_*` / `finalize_issues` / `sort_issues` /
    `write_issues_csv` imports were pruned.
  - **M0.26 — 7 SFTR CLI commands migrated.** Mirrors M0.25 across
    every SFTR scan path (`run_{scan, missing_collateral, reconcile,
    tr_state_scan, tr_activity_scan, tr_audit, book_reconcile}`).
  - **M0.27 — Web-UI `finalize_artifacts` chokepoint flipped.** The
    `opendqi-server` shared finalisation step now consumes a
    `SortedIssueSink` (its `finish()` replaces the former
    `build_summary` / `build_feedback_summary`); ~14 `run_*_server`
    handlers + `run_server_scan` push into a per-request sink (with
    a `Mutex` when a parallel `stream_checks_into` is used). Verified
    via the 16 `opendqi-server` integration tests + byte-equivalence
    by construction (same core path the CLI goldens already lock).

  Output invariants used (all pre-proven): the no-spill sink path is
  literally `finalize_issues` (M0.22, test-locked); `sink.finish` ==
  the former `build_summary` (M0.21 — `quality_score` delegates to
  `quality_score_from_counts`; `IssueAggregator::observe` == the
  manual loop); `issue_cmp`-equal ⟹ identical CSV row so push order
  is immaterial. Push-order invariants kept goldens byte-identical
  across all 19 cases without `UPDATE_GOLDEN`.

- **`IssueAggregator` — online summary; 18 `build_summary` copies
  de-duplicated (Milestone 0.21; Increment A of the streaming issue
  pipeline).** New `opendqi_core::IssueAggregator` computes
  `ScanSummary` (severity/dimension counts, total, `quality_score`)
  from a *stream* of issues without retaining them;
  `scoring::quality_score` now delegates to a shared
  `quality_score_from_counts` so the score is reproducible from
  counts alone. The 17 CLI + 1 server hand-rolled `build_summary`
  bodies collapse to thin adapters over it. Output byte-identical
  (same arithmetic — zero golden/conformance diff; preflight green).
  Foundational seam for the streaming pipeline that replaced the
  resident `Vec<DqIssue>` (M0.22–0.27 above); independently valuable
  as de-dup.

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
  for the structural cleanliness: output byte-identical (zero
  golden/conformance diff — append order irrelevant,
  `finalize_issues` unchanged; M0.19 unit tests pass as-is),
  preflight green. Not a perf win — stated plainly
  (M0.14/M0.17/M0.19 discipline). *Now realised by M0.21–0.27 above.*

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
  collect tweak moves it. Kept solely for the de-duplication: output
  byte-identical (zero golden/conformance diff — the collect is
  order-preserving, `finalize_issues` unchanged), preflight green.
  Not a perf win — stated plainly (M0.14/M0.17 discipline). *Now
  realised by M0.21–0.27 above.*

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
  transient** (subsequently the M0.21–0.27 target). Shipped as a
  correct, necessary structural fix — **not** a peak win on its own;
  SFTR unaffected. **Behaviour:** `issues.csv` tie-order is now
  content-deterministic (was the parallel-insertion artifact); a
  single golden (`sftr-reconcile.issues.csv`) regenerated — a proven
  pure permutation (identical row set, zero rows added/removed/
  modified, `*.summary.json` byte-unchanged). `EvidenceItem` gains a
  derived `PartialOrd, Ord` (additive, not serialised).

### Fixed

- **`scripts/bench-scale.sh` — `set -u` empty-array crash when
  `--mem-trace` not passed (tooling).** The script blew up on macOS
  Bash 3.2 with `MT_ENV: unbound variable` whenever `--mem-trace`
  was omitted, because `"${MT_ENV[@]}"` is not safe under
  `set -u` for an empty array. Switched to the canonical
  `${MT_ENV[@]+"${MT_ENV[@]}"}` empty-safe expansion. Tooling-only
  fix.

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

[Unreleased]: https://github.com/PauFou/OpenDQI/compare/v0.12.3...HEAD
[0.12.3]: https://github.com/PauFou/OpenDQI/compare/v0.12.2...v0.12.3
[0.12.2]: https://github.com/PauFou/OpenDQI/compare/v0.12.1...v0.12.2
[0.12.1]: https://github.com/PauFou/OpenDQI/compare/v0.12.0...v0.12.1
[0.12.0]: https://github.com/PauFou/OpenDQI/compare/v0.11.0...v0.12.0
[0.11.0]: https://github.com/PauFou/OpenDQI/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/PauFou/OpenDQI/compare/v0.9.0...v0.10.0
[0.9.0]: https://github.com/PauFou/OpenDQI/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/PauFou/OpenDQI/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/PauFou/OpenDQI/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/PauFou/OpenDQI/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/PauFou/OpenDQI/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/PauFou/OpenDQI/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/PauFou/OpenDQI/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/PauFou/OpenDQI/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/PauFou/OpenDQI/releases/tag/v0.1.0
