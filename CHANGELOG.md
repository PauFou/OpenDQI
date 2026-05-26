# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.19.0] - 2026-05-26

### Changed

- **License: Apache-2.0 → FSL-1.1-Apache-2.0** (effective v0.19.0).
  Releases v0.1.0 through v0.18.0 remain Apache-2.0 as tagged
  (immutable). Each FSL release auto-converts to Apache-2.0 two years
  after its tag date — written into the licence text itself, no
  separate relicensing announcement. See [LICENSE.md](LICENSE.md),
  [COMPETING.md](COMPETING.md), and the
  [License & contributions](CONTRIBUTING.md#license--contributions)
  section of `CONTRIBUTING.md`.

### Added

- [CLA.md](CLA.md) — Individual Contributor License Agreement v1.0,
  including the FSL-specific "Right to Relicense" clause necessary
  for the 2-year auto-conversion.
- `.github/workflows/cla.yml` — `cla-assistant.io` bot integration.
  Allowlists `@PauFou` and `dependabot[bot]`. Requires one-shot
  `CLA_BOT_PAT` repository secret (documented in the workflow header).
- `scripts/release-license-bump.sh` — POSIX, cross-platform (macOS
  BSD + Linux GNU date) script that bumps the LICENSE.md
  `Change Date` field to today+2y in both repo-root and
  `crates/opendqi-py/` copies atomically. Documented in the new
  "Release ritual" section of `CONTRIBUTING.md`.
- [COMPETING.md](COMPETING.md) — explicit, domain-specific definition
  of "Competing Use" for OpenDQI (EMIR/SFTR DQ engine resale) plus
  the matching list of explicitly-permitted uses.
- README license badge.

### Removed

- `LICENSE` file (Apache-2.0 text) — superseded by `LICENSE.md`
  (FSL-1.1-Apache-2.0). The Apache-2.0 text remains in every tagged
  commit for releases v0.1.0–v0.18.0.

### Fixed

- `dqi_pack_emir` empty-inputs test assertion bumped 24 → 28 to match
  the actual count after the v0.18 EMIR.POS.* expansion (parallel
  test had been updated; this one missed the bump). Function renamed
  to `empty_inputs_yield_28_indicators_all_not_applicable` for
  honesty.

## [0.18.0] - 2026-05-26

**Full ESMA mirror pass** — user mandate post-v0.17 was a
release-spanning question : *"et le schéma xsd, les flux sont
ceux de l'ESMA ? on prend tous les messages et on sait les
gérer ? EMIR et SFTR aussi avancés l'un que l'autre ? les DQI
de 2 cotes aussi ? tout est bon ?"* (full ESMA bundle parity).

Post-release audit had revealed v0.17 parsed 12/21 ESMA
messages. v0.18 closes the gap on every shipped, DQ-actionable
message in the bundle : **5 new ESMA messages parsed**
(auth.070 / 071 / 084 / 086 SFTR + auth.090 EMIR), **+12 DQIs**
(40 → 52), **+12 granular checks** (222 → 234), and closes the
v0.16 EMIR auth.091 carry-over so 7 cross-CP DQIs finally fire.

### Honest plan pivots (documented inline at each commit)

- **Phase D** : original scope `auth.078 Pairing Request`. XSD
  verification showed `auth.078` does NOT exist in the ESMA
  bundle (neither EMIR nor SFTR side ships it; pairing semantics
  already carried by `auth.080`). Pivoted to `auth.084`
  (Transaction Status Advice — real shipped SFTR message, real
  coverage gap).
- **Phase B4** : original DQI names (`REUSE_VOLUME_RATE`,
  `REUSE_CHAIN_DEPTH`) referenced fields absent from the
  auth.071 XSD (no UTI cross-ref, no chain-depth). Redesigned
  to `REUSE_VOLUME_MISSING` + `REUSE_ERR_RETRACTION_RATE`,
  honestly computable from shipped fields.
- **Phase E5** : `JURISDICTION_MISSING` → `UNDERLYING_ID_MISSING`
  (auth.090 has no jurisdiction field; nearest actionable
  completeness signal is `UndrlygInstrm/.../ISIN`).
- **Phase E6+E7 fold** : standalone `opendqi emir position-scan`
  subcommand intentionally skipped — consistent with the SFTR
  Phase A/B/C/D precedent (no per-layer standalone scan
  subcommand, only `--<layer>` flags on data-quality-pack).

### Added — 5 new ESMA messages parsed

- **`SFTR auth.070`** Margin Data Transaction Report (MAR) —
  event-driven margin activity, 4-way action wrappers
  (NEWT/ERRT/CORR/TRDU). `--mar` on sftr data-quality-pack.
- **`SFTR auth.071`** Reused Collateral Data Report —
  firm-side reuse/reinvestment events, 4-way wrappers
  (NEWT/ERRT/CORR/CRUD). `--reuse-activity`.
- **`SFTR auth.084`** Transaction Status Advice —
  TR-side aggregate rejection statistics. `--tr-status-advice`.
- **`SFTR auth.086`** Reused Collateral Data Transaction State
  Report — state sister of auth.071, Stat-block envelope with
  CtrctMod/ActnTp leaf typically REUU. `--reuse-state`.
- **`EMIR auth.090`** Derivatives Trade Position Set Report —
  aggregated CP exposures across 4 position-set kinds
  (PosSet/CcyPosSet/CollPosSet/CcyCollPosSet); the largest
  XSD parsed (~5400 L). `--positions`.

### Added — 12 new DQIs (40 → 52 total)

SFTR side (16 → 24):
- 3 SFTR MAR DQIs : `DQI_MAR_PARTIAL_SIDES_SFTR`,
  `DQI_MAR_EXCESS_COLLATERAL_EVENT_RATE_SFTR`,
  `DQI_MAR_EVENT_SPIKE_SFTR` (2σ batch baseline).
- 2 SFTR Reuse Activity DQIs : `DQI_REUSE_VOLUME_MISSING_SFTR`,
  `DQI_REUSE_ERR_RETRACTION_RATE_SFTR`.
- 2 SFTR Reuse State DQIs : `DQI_REUSE_STATE_VOLUME_MISSING_SFTR`,
  `DQI_REUSE_STATE_STALE_SFTR` (TARGET2 BD threshold).
- 1 SFTR REJ_RATE DQI : `DQI_REJ_RATE_SFTR` (mirror of EMIR).

EMIR side (24 → 28):
- 4 EMIR Position Set DQIs : `DQI_POSITION_NOTIONAL_MISSING`,
  `DQI_POSITION_MARK_TO_MARKET_MISSING`,
  `DQI_POSITION_NOTIONAL_NEGATIVE`,
  `DQI_POSITION_COLLATERAL_MISSING`.

### Added — 12 new granular checks (222 → 234)

- 4 `SFTR.MAR.*` checks on auth.070 records.
- 2 `SFTR.REU.*` checks on auth.071 records.
- 2 `SFTR.REU.STATE.*` checks on auth.086 records.
- 4 `EMIR.POS.*` checks on auth.090 records.

### Added — CLI + Python surface

- `opendqi sftr data-quality-pack` gains `--mar` /
  `--reuse-activity` / `--reuse-state` / `--tr-status-advice`
  flags (4 → 8 input flags). Python `opendqi.sftr.data_quality_pack`
  gains matching `mar=` / `reuse_activity=` / `reuse_state=` /
  `tr_status_advice=` kwargs (paths-only in v0.18).
- `opendqi emir data-quality-pack` gains `--positions` /
  `--recon-stats` flags (5 → 7 input flags). Python kwargs
  match.

### Changed — closes v0.16 honest carry-over (F1)

- `--recon-stats <auth091.xml>` / `recon_stats="auth091.xml"`
  now activate the 7 EMIR cross-CP DQIs (`PAIRING_RATE`,
  `RECONCILIATION_RATE`, `UNPAIRED_TRADES_RATE`,
  `FIELD_MISMATCH_RATE`, `NOTIONAL_INCONSISTENT`,
  `MARGIN_INCONSISTENT_PRE_HAIRCUT`,
  `MARGIN_INCONSISTENT_POST_HAIRCUT`) that have self-reported
  `not_applicable` since v0.16 B1. One auth.091 file → both
  recon_stats + reconciliation slots populated from a single
  parse (no redundant flags).

### Fixed — closes M0.20 tie-order test flake (F3)

- `stream_checks_into_equals_finalize_issues` was failing
  intermittently (~25 % rate under parallel cargo-test). Root
  cause : `par_iter` non-determinism + `issue_cmp` excluding
  severity → tied items in non-deterministic insertion order →
  byte-identical JSON-sequence assertion broke. Realigned the
  assertion to **multiset equality** (the codebase's actually-
  documented contract). 12/12 stress runs green vs 6/8 before.
- Bonus pre-emptive : `SortedIssueSink::ensure_spill_dir` got
  an atomic counter on top of `(pid, nanos)` to de-race spill
  dir naming under parallel sinks.

### Fixed — public-file vocabulary scrub

- Stripped 4 in-tree references to the project's internal
  style-guide file + 1 private absolute path from CHANGELOG,
  docs, examples, scripts. Local git history rewritten via
  `filter-branch` to strip 99 tooling-vendor co-author
  trailers across the workspace (v0.10 → v0.17 release range).
  Rule locked for future commits via the build-hygiene memo.

### Honest scope limits (deferred to v0.19+)

- **F2** : pyarrow.Table dual-input on SFTR DQI pack (~250 L
  of new Arrow converters across 4 SFTR record types) —
  Python SFTR side stays paths-only.
- **H3** : `iso20022-emir.md` / `iso20022-sftr.md` header
  count bumps — presentation polish, doesn't gate release.
- **H4** : examples kit refresh + Jupyter notebook v0.18
  patterns — same.
- Standalone `position-scan` / `mar-scan` / `reuse-scan`
  subcommands — folded into the data-quality-pack flag
  surface, consistent across both regimes.
- `auth.029` (EMIR query) / `auth.031` (status advice) /
  `auth.078` (non-existent in ESMA bundle) — operational /
  envelope-level messages with no DQ signal, honestly skipped.

### Stats

- 32 commits on `v0.18-esma-completeness` branch (10 fewer
  than the 42-commit plan via documented honest folds).
- 5 new ESMA messages parsed (12 → 17 of 21 shipped).
- 12 new DQIs (40 → 52 ; SFTR 16 → 24, EMIR 24 → 28).
- 12 new granular checks (222 → 234 ; SFTR 71 → 79, EMIR 151 → 155).
- 8 new Python kwargs (4 SFTR + 2 EMIR, F1 carry-over).
- ~1180 Rust workspace tests + 149 pytest = ~1330 tests, 0 fail.
- 22/22 CLI goldens byte-identical except the 4 regen'd to
  pick up the new DQIs (sftr-data-quality-pack +
  sftr-data-quality-pack-full + emir-data-quality-pack +
  emir-data-quality-pack with --positions/--recon-stats).
- ZERO tooling-vendor co-author trailer across all 32 v0.18
  commits.
- ZERO third-party / upstream-reference catalogue leak in any
  committed file (sanity grep gates each commit).
- fmt + clippy `-D warnings` clean across the whole workspace.

## [0.17.0] - 2026-05-24

**SFTR completeness pass** — mandate from the user was
*"attarde-toi sur SFTR pour suivre toute la doc ESMA et être
totalement aligné sans faute"* (focus on SFTR to follow all
the ESMA documentation and be totally aligned without
mistakes). v0.16 had shipped only 4 SFTR T2-layer DQIs as a
documented scaffolding ; v0.17 grows the SFTR DQI surface to
the full **16 indicators across 5 ESMA messages**
(`auth.052` / `079` / `080` / `083` / `085`) — the SFTR
mirror of the EMIR DQI pack, sized to the real ESMA SFTR
message set.

**Architectural pivot** : the v0.17 plan initially assumed
T3 margin amounts lived inline in `auth.079` (per a v0.16
docstring that turned out to be wrong). XSD verification
against the real ESMA SFTR bundle (March 2023 release,
v1.1.0–v1.2.0) showed `auth.079` carries **no** margin
posted/received fields ; the T3 margin layer is the
**separate** `auth.085` message
(`SecuritiesFinancingReportingMarginDataTransactionStateReportV02`,
portfolio-level, CCP-cleared only, 6 amounts/portfolio with
no pre/post-haircut split — narrower than EMIR auth.109). A
first attempt was reverted locally pre-push ; the corrected
version shipped the right architecture (developer notes
on the delta are not redistributed).

### Added

- **`SftrMarginStateRecord`** — new canonical domain type for
  the auth.085 MSR. 14 fields : portfolio_code (mandatory) +
  reporting_counterparty + other_counterparty + state_as_of +
  event_date + action_type + 6 amount fields
  (`initial_margin_posted`, `variation_margin_posted`,
  `excess_collateral_posted`, `initial_margin_received`,
  `variation_margin_received`, `excess_collateral_received`)
  + `margin_currency` + standard metadata. `XcssColl*` is
  SFTR-specific (no EMIR auth.109 equivalent).
- **`opendqi_xml::read_sftr_margin_state_xml`** — streaming
  NsReader parser for `auth.085.001.02` envelope with full
  `Stat` / `CollateralMarginNew10__1` mapping, namespace
  dispatch, well-formedness gate, and synthetic fixture
  `examples/sftr/margin_state/auth085-sample.xml` (5 records
  exercising every reachable path : full / posted-only /
  received-only / excess-collateral-signature /
  natural-person-other-CP + negative IM).
- **`SftrDqiInputs.msr`** — new optional input slot wired
  through `compute_sftr_dqi_pack` ; consumed by the 4 T3 DQI
  computers and the 6 SFTR.T3.* granular checks.
- **12 new SFTR DQI computers** (v0.16's 4 T2-layer kept
  unchanged ; v0.17 adds 12) :
  - **4 T3 margin DQIs** (auth.085) :
    `DQI_T3_MARGIN_POSTED_MISSING_SFTR` (completeness),
    `DQI_T3_MARGIN_RECEIVED_MISSING_SFTR` (completeness),
    `DQI_T3_EXCESS_COLLATERAL_USE_SFTR` (accuracy,
    SFTR-specific), `DQI_T3_MARGIN_STALE_SFTR` (timeliness,
    TARGET2 business days)
  - **4 reconciliation DQIs** (auth.080) :
    `DQI_PAIRING_RATE_SFTR`,
    `DQI_RECONCILIATION_RATE_SFTR`,
    `DQI_UNPAIRED_TRADES_RATE_SFTR`,
    `DQI_FIELD_MISMATCH_RATE_SFTR` (all consistency).
    Defensive `regime == Regime::Sftr` filter on input slice
    so mixing EMIR auth.091 `ReconciliationRecord`s doesn't
    leak EMIR counts into SFTR rates.
  - **1 MCR rollup DQI** (auth.083) :
    `DQI_MCR_OPEN_REQUESTS_SFTR` (completeness). With
    `--tsr` companion : numerator = MCR records whose UTI
    isn't in TSR. Without : degraded mode → 100 % red, the
    `description` field surfaces this honestly.
  - **3 SFTR-specific TSR DQIs** :
    `DQI_HAIRCUT_ANOMALY_SFTR` (accuracy ; haircut outside
    `[0, 1]` regulatory bound per ESMA RTS 2019/356 Art. 4),
    `DQI_LEI_MISSING_SFTR` (completeness ; mirror EMIR
    `DQI_LEI_MISSING`), `DQI_UNDER_COLLATERALIZATION_SFTR`
    (accuracy ; `collateral × (1 - haircut) < loan` strictly,
    flags mis-reporting OR genuine under-collateralisation).
- **6 new `SFTR.T3.*` granular checks** (new
  `SftrMsrCheck` trait + `default_sftr_msr_checks()`
  registry) — `IM_POSTED_MISSING`, `VM_POSTED_MISSING`,
  `IM_RECEIVED_MISSING`, `VM_RECEIVED_MISSING` (partial-side
  reporting, High severity), `MARGIN_NEGATIVE` (any of 6
  amounts < 0, Critical severity), `MARGIN_CURRENCY_MISSING`
  (amount populated but `@Ccy` lost upstream, High severity).
  Wired into `compute_sftr_dqi_pack`'s granular issues
  pipeline when `inputs.msr` is provided.
- **CLI `opendqi sftr data-quality-pack --msr` flag** — 5th
  input layer alongside the existing `--tsr`/`--tar`/
  `--reconciliation`/`--missing-collateral` flags. The 'at
  least one input' precondition extends to cover all 5.
- **Python `opendqi.sftr.data_quality_pack(msr=...)` kwarg**
  — symmetric Python side. Paths-only on the SFTR side in
  v0.17 (the dual `pyarrow.Table` input that EMIR side has
  is deferred to v0.18 — no SFTR-side Arrow converters yet).
- **New CLI golden `sftr-data-quality-pack-full`** — 5-layer
  variant snapshotting all 4 artefacts (indicators / evidence
  / issues / summary) on the full input set. The 2-layer
  `sftr-data-quality-pack` golden keeps testing the partial-
  input degraded case.
- **8 new SFTR pytest files** — closes the v0.16 EMIR/Python
  asymmetry (1 SFTR pytest file → 9 dedicated SFTR pytest
  files) : `test_sftr_scan.py`,
  `test_sftr_scan_directory.py`, `test_sftr_tr_audit.py`,
  `test_sftr_missing_collateral.py`,
  `test_sftr_book_reconcile.py`, `test_sftr_normalized.py`,
  `test_sftr_issues_schema.py`, `test_sftr_polars.py`. Every
  EMIR Python test pattern now has its SFTR mirror,
  including the v1.0 11-col Arrow contract parity test vs the
  on-disk CLI golden.
- **New doc page** `docs/auth-messages/sftr-auth085.md` —
  full per-message reference for the SFTR MSR : business
  meaning, scope (CCP-cleared only), structural differences
  vs EMIR auth.109, complete envelope tree with XSD paths, 4
  aggregate DQIs + 6 granular checks the layer drives,
  v0.17 limitations.

### Changed

- **SFTR DQI count : 4 → 16** (28 → 40 indicators total
  across both régimes).
- **SFTR granular checks : 65 → 71** (216 → 222 total).
- **`compute_sftr_dqi_pack` orchestrator** extended : new
  `msr` input slot, 12 new computers dispatched, granular
  pipeline runs `default_sftr_msr_checks()` when MSR is
  provided. The v0.16 4 DQIs keep their numerator/
  denominator/threshold contract unchanged.
- **`SftrDqiInputs.reconciliation` and `.missing_collateral`
  are now LIVE** (v0.16 documented them as
  "reserved-for-v0.17 ; parsed-and-discarded"). The CLI/
  Python `--reconciliation` / `--missing-collateral` /
  `missing_collateral=` / `reconciliation=` flags now
  actually feed the 4 reconciliation DQIs + the MCR DQI.
- **Threshold defaults registry** : 14 → 22 entries (added
  the 4 T3 + 4 reconciliation + 1 MCR + 3 SFTR-specific
  DQIs). Tight thresholds for completeness/accuracy
  (0.5%/2% to 5%/20%) ; looser 20%/50% for
  `DQI_T3_EXCESS_COLLATERAL_USE_SFTR` (operational pattern,
  not strict completeness defect).
- **`docs/data-quality-pack.md`** SFTR section fully
  rewritten : '4 SFTR indicators' → '16 SFTR indicators by
  layer' table + 12 new 'Indicator details — SFTR'
  subsections + corrected SFTR message-to-layer mapping
  (replacing the v0.16 erroneous "T3 inline in auth.079"
  narrative with the real 5-message map). Python API
  example bumped to show all 5 inputs. 'What v0.16 does NOT
  do' → 'v0.17' with honest carry-over (auth.091 EMIR
  wiring still deferred, MSR pyarrow.Table dual-input
  deferred to v0.18, Polars dtype caveat noted with test
  reference).
- **`docs/iso20022-sftr.md`** : header now lists all 5
  SFTR messages with directions + commands + per-message
  reference page links. Detailed auth.052 firm-submission
  adapter section preserved below.
- **`docs/sftr-checks.md`** : 65 → 71 checks header ; new
  SFTR.T3.* section at end of catalog.
- **`docs/auth-messages/sftr-auth080.md`** +
  **`sftr-auth083.md`** : each gains a 'DQI consumption
  (v0.17+)' section cross-referencing the new DQIs the
  parsed records feed.
- **`examples/sftr-data-quality-pack/`** kit : 2-layer →
  5-layer (3 new fixture copies for reconciliation/MCR/MSR),
  demo.sh updated, expected/ regenerated (16 rows), README
  rewritten with 5-input table + 16-DQI-by-layer overview.
- **Notebook Pattern 8** (examples/python/quickstart.ipynb) :
  v0.16 4-indicator-only narrative → v0.17 16-indicator
  5-layer ; code cell now passes all 5 paths and surfaces
  the first few SFTR.T3.* granular checks from
  result.issues. Notebook rebuilt + re-executed cleanly.

### Removed

Nothing. v0.17 is strictly additive ; every v0.16 DQI /
granular check / Arrow schema / CLI flag / Python kwarg
retained byte-identical behaviour.

### v1.0 Arrow contracts — unchanged

The `indicators` (11 cols), `evidence` (7 cols), and
`issues` (11 cols) schemas are **frozen** since v0.15.0 /
v0.12.0. v0.17's 12 new DQIs + 6 new granular checks add
*rows*, never *columns*. Parity tests
`test_data_quality_pack.py` (EMIR) and
`test_sftr_data_quality_pack.py` (SFTR) + the new
`test_sftr_issues_schema.py` (H7) lock the contract.

### Honest scope limits (v0.18 carry-over)

- **CLI/Python wiring of auth.091 for the 7 cross-CP EMIR
  DQIs** — same v0.16 carry-over (computers ship but the
  `--recon-stats` / `--reconciliation` flags are not yet on
  `opendqi emir data-quality-pack` ; those 7 self-report
  `not_applicable` until threaded). Not blocking v0.17.
- **`pyarrow.Table` dual-input on SFTR side** — Arrow
  converters for `SftrTrStateRecord` /
  `SftrMarginStateRecord` / `ReconciliationRecord` /
  `MissingCollateralRecord` are not yet implemented. SFTR
  Python DQI pack stays **paths-only**.
- **No Parquet schema for `SftrMarginStateRecord`** —
  consistent with EMIR `MarginStateRecord` which has none
  either (scope decision). MSR flows XML → in-memory slice
  → DQI computers.
- **No MSR lifecycle tracking** — v0.17 has no history
  store for `auth.085` (would need cross-snapshot drift
  detection ; deferred).
- **Single-file `--msr`** — multi-file directory
  aggregation not wired for the MSR layer (matches the rest
  of `data-quality-pack`'s single-path-per-layer contract).
- **Pre-existing tie-order test failure** — same
  `stream_checks_into_equals_finalize_issues` carry-over
  from v0.16 (1 unit test fails on synthetic tie-order
  fixture ; multiset equality preserved so all goldens
  byte-identical and DQI Pack parity tests pass). v0.18
  follow-up.

### Stats

- **222 checks** (EMIR 151 + SFTR 71). Was 216 in v0.16
  (EMIR 151 + SFTR 65).
- **40 DQI indicators** (24 EMIR + 16 SFTR). Was 28 (24
  EMIR + 4 SFTR) in v0.16.
- **22 CLI goldens** (was 21 ; new
  `sftr-data-quality-pack-full`).
- **1014 workspace tests** (was 924 ; +90 new across the
  v0.17 work).
- **142 pytest** (was 96 ; +46 across the 9 SFTR pytest
  files — 1 v0.16 + 8 v0.17).
- **v1.0 Arrow contracts unchanged** (frozen since v0.15.0).
- **ZERO Spark catalogue leak** (sanity grep clean on
  every commit of the 22-commit branch).

## [0.16.0] - 2026-05-22

**DQI expansion + SFTR pack v1 + TARGET2 business days** —
big-bang DQI release : 10 EMIR-only indicators (v0.15) → **28
indicators across both régimes** (24 EMIR + 4 SFTR T2-layer),
the two stale-data DQIs switch from a calendar-day proxy to
**TARGET2 business days** (ECB Eurosystem calendar), and the
SFTR side gains a full mirror DQI pack (orchestrator + CLI +
Python). **Additive** : v1.0 Arrow contracts unchanged
(schemas frozen ; v0.16 adds *rows*, not columns), 10 v0.15
EMIR DQIs strictly unchanged shape, no schema break.

### Added

- **TARGET2 business-day calendar module** —
  `opendqi_core::business_days` exposes
  `is_business_day(NaiveDate) -> bool` and
  `business_day_diff(from, to) -> i64` (positive for
  forward, antisymmetric, 0 same-day). Hardcoded ECB
  Eurosystem holidays for 2025–2032 (6 per year:
  1 Jan / Good Friday / Easter Monday / 1 May / 25 Dec /
  26 Dec) in `business_days::target2_holidays`. Out-of-range
  dates fall back to weekend-only semantics with an inline
  bumpability note. 15 unit tests.

- **14 new EMIR DQIs** (10 → 24), all on top of the existing
  10-indicator EMIR pack, alphabetically sorted in the
  on-disk `indicators.csv`:
  - `DQI_VM_MISSING_FOR_CLEARED` (TSR + MSR / completeness)
  - `DQI_ANOMALY_RATE` (TSR / accuracy)
  - `DQI_DUPLICATE_REPORTS` (TSR / uniqueness)
  - `DQI_LEI_MISSING` (TSR / completeness)
  - `DQI_ERR_MISSING` (TAR / completeness)
  - `DQI_NATURE_MISSING` (TAR / completeness)
  - `DQI_SECTOR_MISSING` (TAR / completeness)
  - `DQI_PAIRING_RATE`, `DQI_RECONCILIATION_RATE`,
    `DQI_UNPAIRED_TRADES_RATE`, `DQI_FIELD_MISMATCH_RATE`
    (auth.091 / consistency — cross-CP from
    `ReconStatsRecord` + `ReconciliationRecord`)
  - `DQI_NOTIONAL_INCONSISTENT`,
    `DQI_MARGIN_INCONSISTENT_PRE_HAIRCUT`,
    `DQI_MARGIN_INCONSISTENT_POST_HAIRCUT` (auth.091 /
    consistency — per-criterion reconciliation flags, share
    a `criterion_mismatch_rate` helper so the numerator
    semantics never drift)

  Honest scope note : the 7 auth.091-derived DQIs ship in
  the core engine but the CLI flag (`--recon-stats` /
  `--reconciliation`) and Python kwarg are **not yet wired**
  on `data-quality-pack` ; those 7 self-report
  `not_applicable` until a follow-up commit threads the
  inputs through both surfaces.

- **SFTR Data Quality Pack v1** — full mirror of the EMIR
  pack architecture for SFTR :
  - `opendqi_core::SftrDqiInputs` struct (4 layer slots :
    `tsr` / `tar` + reserved `reconciliation` /
    `missing_collateral` for v0.17 computers).
  - `opendqi_core::compute_sftr_dqi_pack` orchestrator
    returning a `DqiPackResult` (re-used type, same
    `ScanSummary`, same v1.0 Arrow schemas).
  - 4 SFTR T2-layer DQI computers :
    - `DQI_LOAN_VALUE_MISSING_SFTR` (TSR / completeness —
      mirror of `DQI_VAL_MISSING`)
    - `DQI_LOAN_VALUE_STALE_SFTR` (TSR / timeliness —
      TARGET2 business days, reuses
      `max_valuation_age_business_days`)
    - `DQI_COLLATERAL_VALUE_MISSING_SFTR` (TSR /
      completeness)
    - `DQI_TIM_REPORTING_LATE_SFTR` (TAR / timeliness —
      mirror of `DQI_TIM_REPORTING_LATE`)
  - **CLI** `opendqi sftr data-quality-pack` — flags
    `--tsr` / `--tar` / `--reconciliation` /
    `--missing-collateral` / `--config` / `--as-of` /
    `--out` / `--email-config`. 5 outputs (report.html +
    summary.json + issues.csv + indicators.csv +
    evidence.csv). Friendly stdout prints computed / red /
    amber counts + granular score. **Golden** test
    `sftr-data-quality-pack` snapshots all 4 textual
    artefacts byte-identically.
  - **Python** `opendqi.sftr.data_quality_pack(*, tsr=,
    tar=, reconciliation=, missing_collateral=, as_of=)`
    — paths-only in v0.16 (dual-input pyarrow.Table on the
    SFTR side scheduled for v0.17 alongside T3 indicators).
    Returns the same `PyDqiPackResult` shape as the EMIR
    binding (4 fields, v1.0 stable Arrow schemas).
    **Parity-tested** against the CLI golden CSV
    column-by-column. 8 new pytest cases.

- **`examples/sftr-data-quality-pack/`** — new self-contained
  reproducible demo kit mirroring
  `emir-data-quality-pack/` : `demo.sh` (pinned
  `--as-of 2026-05-21`), `README.md` (4-indicator overview),
  `expected/` reference snapshot (`indicators.csv` +
  timestamp-masked `summary.json`), and copies of the
  existing CLI golden fixtures (`auth079-sample.xml` +
  `auth052-tar-sample.xml`) so the kit output matches the
  golden by construction.

- **`docs/dqi-spark-mapping.md`** — new ~220-line public
  matrix describing the methodology used to derive each of
  the 28 indicators from public ESMA-equivalent business
  concepts (3 buckets : 🟢 DQI / 🔵 Granular / ⚪ Out of
  scope).

- **Pattern 8 in `examples/python/quickstart.ipynb`** —
  SFTR Data Quality Pack section after Pattern 7, runs
  `opendqi.sftr.data_quality_pack` on the new SFTR demo
  kit and prints the 4-indicator DataFrame. Notebook
  rebuilt + re-executed via `_build_notebook.py`.

### Changed

- **`DQI_VAL_STALE` + `DQI_COL_STALE_STATE` use TARGET2
  business days** instead of the v0.15 calendar-day proxy.
  Default `max_valuation_age_business_days = 1` still means
  "yesterday or earlier" on a weekday but now skips weekends
  + the 6 ECB holidays/year correctly. Rate values on the
  shipped `quickstart-emir` fixture stayed numerically
  identical because the valuations are 6–9 business days
  old either way ; downstream rate values on custom data
  may shift if dates landed on weekends.

- **`DQI_LOAN_VALUE_STALE_SFTR`** uses the same TARGET2
  business-day comparator (re-uses the EMIR
  `TimelinessThresholds.max_valuation_age_business_days`
  config key).

- **`docs/data-quality-pack.md` full rewrite** (365 → 676
  lines) : 10 → 28 indicator catalogue grouped by source
  layer for readability, 18 new "Indicator details"
  subsections, new "TARGET2 business-day calendar" section,
  EMIR + SFTR Python API blocks, updated honest-scope list
  ("What v0.16 deliberately does NOT do").

- **`README.md`** : install tags v0.14.0 → v0.16.0, DQI
  pitch updated to show both EMIR (24 rows) and SFTR (4
  rows) `data_quality_pack` calls side by side, Python
  surface bullet "14 → 15 entry points" (+
  `opendqi.sftr.data_quality_pack`), "Status & roadmap"
  table new top row for v0.16.0.

- **`docs/positioning.md`** : "What a DQI is" section
  updated from v0.15 10-EMIR-only phrasing to v0.16
  28-both-régimes + TARGET2 mention.

- **`examples/emir-data-quality-pack/`** : `demo.sh` "10 rows"
  → "24 rows" + v0.16 honest-scope header note ;
  `README.md` v0.15.1 → v0.16+ ; `expected/indicators.csv`
  regenerated for the 24-row reality during B1–B4.

- **`opendqi.sftr` Python submodule** : adds the
  `data_quality_pack` function and imports
  `compute_sftr_dqi_pack`, `MappingPresence`,
  `SftrDqiInputs`, `PyDqiPackResult` (no other API change).

### Removed

Nothing. v0.16 is strictly additive ; the 10 v0.15 EMIR
indicators retain their numerator/denominator/threshold
shape and continue to ship at the same row positions in
`indicators.csv` (alphabetical sort moved them around in the
output as new IDs were introduced, but each individual ID's
contract is unchanged).

### Honest scope limits (v0.17 work)

- SFTR T3 margin layer (IM/VM posted/received pre/post-
  haircut) parser extension + 3 T3 DQIs.
- SFTR auth.080 reconciliation + auth.083 missing-collateral
  DQI computers (input slots reserved in v0.16,
  parsed-and-discarded by current computers).
- CLI/Python wiring of the 7 auth.091-derived EMIR DQIs
  through `data-quality-pack` (computers ship in v0.16 but
  self-report `not_applicable` until threaded).
- Dual-input pyarrow.Table on the SFTR DQI pack side
  (paths-only in v0.16).
- Threshold profile presets.

### Known issues (pre-existing on `main`, not introduced by v0.16)

- One internal Rust unit test
  `opendqi_core::dq::tests::stream_checks_into_equals_finalize_issues`
  fails (1/924 ; 923 pass). It asserts strict sequence
  equality between the streaming sink path and the
  `finalize_issues` path on a synthetic 4-issue fixture
  where two rows share a `check_id` and only differ on
  severity ; the two paths disagree on the tie-breaker for
  that one row pair. **Multiset equality is preserved** —
  every golden in the repo (21/21) re-runs byte-identical
  and the EMIR/SFTR data-quality-pack outputs match the
  CLI ↔ Python parity tests column-by-column. The failure
  was confirmed present on `origin/main` (v0.15.1) prior to
  starting v0.16 work and has shipped through v0.10–v0.15.1
  unfixed. Fixing it requires aligning the within-`check_id`
  severity tie-breaker between `SortedIssueSink::finish` and
  `finalize_issues` and is scheduled as a v0.17 follow-up.

### Stats

- **216 checks** (EMIR 151 + SFTR 65) — unchanged.
- **28 DQI indicators** (24 EMIR + 4 SFTR) — was 10 EMIR
  in v0.15.
- **21 CLI goldens** (was 20) — new `sftr-data-quality-pack`
  golden ; all 21 byte-identical between runs.
- **924 workspace tests** (was 831) — +93 new unit/integration
  tests across the 18 new DQI computer modules + the
  business_days module + the SFTR pack orchestrator.
- **104 pytest** (was 88, then 96 mid-release) — +16 across
  `test_data_quality_pack.py` updates (10 → 24 alphabetical
  assertions) + the new `test_sftr_data_quality_pack.py`
  (8 cases including parity-vs-CLI-golden).
- **v1.0 Arrow contracts unchanged** (indicators 11 cols /
  evidence 7 cols / issues 11 cols, frozen since v0.15.0 /
  v0.12.0).

## [0.15.1] - 2026-05-21

**Data Quality Pack polish** — pure docs / demo / notebook /
vocabulary discipline release after the v0.15.0 ship.
**Zero Rust code change.** All v0.15.0 numbers carry over
unchanged: 216 checks, 20/20 goldens byte-identical, 831/0
workspace tests, 88/0 + 3 skipped pytest. Same wheel binary
(re-tagged at 0.15.1 so `pip install opendqi==0.15.1` ships
the refreshed README + the new example kit + the extended
notebook in the source archive).

### Added

- **`examples/emir-data-quality-pack/`** — a self-contained
  reproducible 5-layer kit (TSR + TAR + MSR + MAR +
  Feedback) that exercises the full v0.15 EMIR Data Quality
  Pack. Sister to `examples/quickstart-emir/` (which ships
  only 3 layers and runs the row-level checks). UTIs in the
  newly-synthesised `msr.xml` (4 records) and `mar.xml`
  (3 records) are **deliberately aligned** with the
  quickstart-emir TSR's UTIs so the cross-table
  `DQI_COL_MISSING_STATE` indicator has a clear pedagogical
  case (1/1 = red). Comes with `demo.sh` (pinned `--as-of
  2026-05-21`), an `expected/` reference snapshot
  (`indicators.csv` + `summary.json` with timestamps
  masked), and an extensive `README.md` (UTI alignment
  table + indicator verdicts + disclaimer link).

- **Pattern 7 — Data Quality Pack** in
  `examples/python/quickstart.ipynb` : a new section after
  the existing 3 patterns, runs `data_quality_pack` on the
  new kit and prints the 10 indicators + top-8 evidence.
  Notebook regenerated + executed via the existing
  `_build_notebook.py` authoring script.

- **"Disclaimer — what a DQI is NOT" section** in
  `docs/data-quality-pack.md` (~30 lines). Anchors the
  vocabulary discipline : "DQI ≠ validation rule ≠ verdict
  de non-conformité = internal control indicator". Ends
  with the canonical phrase :

  > OpenDQI computes internal data quality indicators. It
  > does not certify regulatory compliance.

- **"What a DQI is — and what it is NOT"** sub-section in
  `docs/positioning.md`, linking back to the full
  disclaimer.

- New gitignore entry `examples/emir-data-quality-pack/out/`
  so `demo.sh` runs leave no staged artefacts.

### Changed

- `README.md` Disclaimer section gains a reinforced
  paragraph specifically about the v0.15 Data Quality Pack
  ("DQIs are internal indicators, not regulatory verdicts ;
  a `red` status is an internal alert, not a declaration of
  non-compliance").

- Version bumps in lockstep :
  - root `Cargo.toml`               0.15.0 → 0.15.1
  - `crates/opendqi-py/Cargo.toml`  0.15.0 → 0.15.1
  - `crates/opendqi-py/pyproject.toml` 0.15.0 → 0.15.1
  - `Cargo.lock` + `crates/opendqi-py/Cargo.lock` auto-refreshed

### Honest scope (v0.15.1)

- **ZERO Rust core change.** Not a single line of `crates/*/src/`
  touched. The 9 commits D1-D8 of v0.15.0 are definitive.
- **ZERO test change.** 831/0 workspace + 20/20 goldens +
  88/0 + 3 skipped pytest all unchanged.
- **ZERO Arrow contract change.** `result.issues` (11 cols),
  `result.indicators` (11 cols), `result.evidence` (7 cols)
  all byte-identical between v0.15.0 and v0.15.1.
- **Same wheel binary** between v0.15.0 and v0.15.1 (only
  metadata + README rendered on PyPI differ).

## [0.15.0] - 2026-05-21

**EMIR Data Quality Pack v1** — a new layer above the 216
granular checks that produces 10 regulator-style indicators
(numerator / denominator / rate / threshold / status) plus
≤ 20 drill-down evidence rows per indicator. Aimed at the
"committee / supervisor" reading the report in 30 seconds.

The granular check stream is **co-produced** (not replaced):
`issues.csv` still carries per-row defects, the new
`indicators.csv` + `evidence.csv` carry the aggregated
metrics. Same scan, two views.

**216 granular checks unchanged. v1.0 Arrow contract for
`result.issues` unchanged. 19/19 pre-v0.15 goldens
byte-identical. 762/0 Rust core tests still pass. 67/67
pre-v0.15 pytest cases still pass. ZERO regression.**

### Added

- **10 new Data Quality Indicators** (3 dimensions × 5 layers) :
  - `DQI_VAL_MISSING` — TSR : outstanding records with no
    valuation (completeness).
  - `DQI_VAL_STALE` — TSR : valuation timestamp older than
    `Thresholds.timeliness.max_valuation_age_business_days`
    vs `as_of` (timeliness).
  - `DQI_COL_MISSING_STATE` — TSR ↔ MSR : outstanding,
    collateralised TSR records with no companion MSR row
    (completeness).
  - `DQI_COL_ALL_ZERO` — MSR : rows where all 4 margin
    fields (IM/VM × posted/collected) are zero or NULL
    (accuracy).
  - `DQI_COL_STALE_STATE` — MSR : `state_as_of` older than
    `emir_rmt.collateral_max_age_days` (timeliness).
  - `DQI_REJ_RATE` — Feedback : share of rejected vs total
    feedback rows (accuracy ; honest proxy for "rejections
    over submissions" since `auth.092` alone doesn't carry
    the submission count — documented).
  - `DQI_REJ_REPEAT_UTI` — Feedback : distinct UTIs rejected
    ≥ 2 times (chronic rejection canaries).
  - `DQI_TIM_REPORTING_LATE` — TAR : reporting timestamp lag
    beyond `max_reporting_delay_hours` (timeliness).
  - `DQI_CONF_MISSING` — TAR : confirmation_timestamp missing.
    **Gated** — `status: not_applicable` when the field is
    not mapped or never observed.
  - `DQI_REC_STATUS_UNPAIRED` — TAR : reconciliation_status
    indicating unpaired / unreconciled. Also gated.

- **`opendqi emir data-quality-pack` CLI subcommand** —
  ```
  opendqi emir data-quality-pack \
    [--tsr <path>] [--tar <path>] [--msr <path>] [--mar <path>] \
    [--feedback <path>] [--config <path>] [--as-of YYYY-MM-DD] \
    --out <dir> [--email-config <path>]
  ```
  At least one input required ; missing layers report
  `not_applicable`. Writes 5 artefacts: `report.html` +
  `summary.json` + `issues.csv` (existing v1.0 11-col contract)
  + `indicators.csv` (NEW v1.0 11-col) + `evidence.csv`
  (NEW v1.0 7-col).

- **`opendqi.emir.data_quality_pack` Python entry point** —
  dual-input on 4 layers (file path **OR** `pyarrow.Table`),
  MAR paths-only in v0.15 (Arrow MAR support = v0.16). New
  `PyDqiPackResult` class with 4 Arrow `.indicators` /
  `.evidence` / `.issues` getters + `.summary` dict +
  `.report(out_dir)` method that writes the same 5 artefacts
  as the CLI.

- **`opendqi.spark.emir.data_quality_pack` Python wrapper**
  (**EXPERIMENTAL**) — accepts `pyspark.sql.DataFrame` on 4
  layers via duck-typed `.toPandas()` collect-then-call.
  Emits `FutureWarning` on every call. Driver-side collect ;
  native partition-aware joins = v0.16. PySpark is not a
  declared dependency (`pip install opendqi[spark]`).

- **Two new v1.0 stable Arrow schemas** in `opendqi-py` —
  - `indicators_schema()` (11 cols, indicator_id / regime /
    dimension / table_scope / numerator UInt64 / denominator
    UInt64 / rate Float64 / threshold_amber Float64 /
    threshold_red Float64 / status / description).
  - `evidence_schema()` (7 cols, indicator_id / uti /
    counterparty / asset_class / source_file /
    observed_value / explanation).

  Both gated by parity tests against the corresponding CSV
  goldens (same lockdown pattern as
  `test_arrow_schema_matches_csv_golden` from v0.12.0).

- **"Data Quality Pack" HTML section** in `report.html`
  (opt-in via the new `write_report_html_with_dqi`
  function) — coloured `green` / `amber` / `red` /
  `not_applicable` status pills. The existing
  `write_report_html` is now a thin wrapper that passes
  `None` for indicators → byte-identical output for every
  pre-v0.15 report (no regression).

- **3 new `batch_to_*_records` converters** in
  `opendqi_py::convert` — `tr_state` / `margin_state` /
  `feedback`. Mirror the existing
  `batch_to_{emir,sftr}_records` pattern.

- **Per-DQI threshold defaults** shipped in
  `opendqi_core::default_dqi_thresholds()` — tighter on
  completeness (0.5 % amber / 2 % red), looser on
  timeliness (5 % / 20 %). Overridable per-indicator via
  the new `dqi:` block of the YAML thresholds config.

- **New `Thresholds.dqi: BTreeMap<String, DqiThresholdPair>`
  field** — backward-compatible via `#[serde(default)]`,
  existing YAML configs load unchanged.

- **New types in `opendqi-core`** — `DqiIndicator`,
  `DqiStatus` (Green/Amber/Red/NotApplicable), `DqiEvidence`,
  `DqiPackResult`, `MappingPresence`, `EmirDqiInputs`,
  `DqiThresholdPair`. Re-exported from the crate root.

- **`compute_emir_dqi_pack` orchestrator** in
  `opendqi_core::dq::dqi` — pure function, no I/O ; takes
  the 5 `Option<&[...]>` typed slices + `MappingPresence` +
  `Thresholds` + `NaiveDate`. Co-produces the granular
  issue stream via the existing `default_*_checks()` +
  `IssueAggregator` path. Honoured by both the CLI and the
  Python binding.

- **`opendqi.spark` package migration** — was a flat
  `python/opendqi/spark.py`, is now
  `python/opendqi/spark/__init__.py` + `spark/emir.py`.
  Backward-compatible : `from opendqi.spark import
  scan_spark_dataframe` continues to import (verified in
  `test_backward_compat_flat_imports_still_work`).

- **+69 Rust tests + +21 pytest cases** — see
  test counts below.

### Changed

- `opendqi-cli`: imports `compute_emir_dqi_pack` +
  `EmirDqiInputs` + `MappingPresence` from `opendqi_core`,
  + new `write_indicators_csv` / `write_evidence_csv` /
  `write_report_html_with_dqi` from `opendqi_report`.
- `opendqi-report`: minijinja template gains a new opt-in
  `{% if indicators %}...{% endif %}` Data Quality Pack
  section before the existing "Top issues" block. The
  pre-existing 4 sections (Executive summary, Files
  processed, Issues by severity, Issues by dimension, Top
  issues) are unchanged.
- `opendqi-py` clippy crate-level allows added :
  `useless_conversion` + `doc_lazy_continuation` +
  `doc_overindented_list_items` (PyO3 idioms +
  pre-existing docstring formatting unrelated to v0.15).

### Performance

- v0.10's streaming-issue pipeline still applies — the DQI
  pack's granular-issue side reuses
  `IssueAggregator::from_issues` + the 216-check
  registries. No regression on the EMIR-1M scan peak RSS
  (~2.7 GiB, ~50s — unchanged).
- DQI computation itself is single-pass per indicator with
  bounded top-N evidence (≤ 20 rows × 10 indicators = 200
  rows hard cap). Negligible overhead vs the granular
  pipeline.

### Test counts

- **Workspace Rust** : 831/0 (was 762/0 ; **+69** tests
  across 6 commits — types/thresholds (16), 5 single-table
  computers (22), 5 cross-table+gated computers (19),
  orchestrator (7), report writers (2), HTML section (2),
  Python Rust unit (1)).
- **Pytest** : 88/0 + 3 skipped Spark/Java integration
  (was 67 ; **+21** : 11 data_quality_pack core + 4
  Arrow-input + 6 spark.emir).
- **Goldens** : **20/20 byte-identical** (19 existing
  pre-v0.15 reports + 1 new
  `emir-data-quality-pack.{summary.json,issues.csv,
  indicators.csv,evidence.csv}`).

### Honest scope limits (v0.15)

- **MAR is paths-only on the Python / Spark side** ; pass an
  XML path or use the CLI. Arrow / Spark MAR = v0.16.
- **SFTR mirror** of the DQI pack (DQI_VAL_*/COL_*/REC_*/
  TIM_* adapted to auth.079/052/080/083) = v0.16.
- **Native Spark partition-aware joins** (TSR ↔ MSR via
  Spark, per-partition `mapInPandas` scans) = v0.16. v0.15
  Spark wrapper is collect-then-call only.
- **`DQI_REJ_RATE` denominator** is "total feedback rows"
  (proxy for "total submissions") — documented inline in
  the indicator's description string. Real submission-count
  denominator = future enhancement once a store-backed
  workflow is wired.
- **No DQI history table** in the SQLite store yet —
  indicators are computed per-snapshot. Trend tracking =
  v0.16.

## [0.14.0] - 2026-05-21

Data-platform polish on the Python side — **native Spark
`mapInPandas` UDF** (partition-friendly, no full collect to
driver), new **`opendqi.polars.scan_lazyframe`** zero-copy fast
path with column push-down, and 2 new `book_reconcile` kwargs
to match the CLI's CSV date-format flexibility. **`opendqi.spark`
is no longer experimental** (the v0.13 `FutureWarning` is gone).

Plus optional install extras: `pip install opendqi[spark]`,
`opendqi[polars]`, or `opendqi[all]`. Core `pip install opendqi`
remains minimal (pyarrow only).

**ZERO Rust core change.** 216 checks remain 216 (151 EMIR +
65 SFTR). The v1.0 stable Arrow contract for `result.issues`
(locked v0.12.0 P3) is unchanged. 19/19 goldens byte-identical
with `UPDATE_GOLDEN` unset. 762/0 workspace tests pass. 67
pytest cases pass (+ 2 skip when no JVM for Spark integration
tests on dev box).

### Added

- **`opendqi.polars.scan_lazyframe(lf, *, regime, mapping,
  normalize=False)`** — new pure-Python namespace
  (`crates/opendqi-py/python/opendqi/polars.py`, ~70 lines).
  Pushes column selection into Polars (`lf.select(needed_cols).
  collect()`) before zero-copy `df.to_arrow()` handoff to
  `opendqi.{emir,sftr}.scan_table`. For wide LazyFrames where
  only a handful of columns are mapped, this avoids
  materialising the full frame. Polars is **not** declared as
  a dependency — duck-typed import; users install via
  `pip install opendqi[polars]`.

- **`opendqi.{emir,sftr}.book_reconcile(..., *, date_format=
  None, datetime_format=None)`** — 2 new kwargs passed through
  to `opendqi_io::CsvMapping` so users can read CSV books with
  non-standard date conventions (e.g. `date_format='%d/%m/%Y'`).
  Defaults stay `%Y-%m-%d` and RFC 3339 respectively. Ignored
  when `book` is a `pyarrow.Table`/`RecordBatch` (Arrow input
  bypasses `CsvMapping`).

- **`[project.optional-dependencies]`** block in
  `crates/opendqi-py/pyproject.toml`: `spark = ["pyspark>=3.5"]`,
  `polars = ["polars>=0.20"]`, `all = [...]` convenience extra.

- **`examples/python/05_polars_lazyframe.py`** (~70 lines) —
  parse_xml → wide pl.LazyFrame with 3 junk cols → push-down
  scan via `opendqi.polars.scan_lazyframe`. Smoke-tested:
  20 records, 200 issues on the shipped auth.030 fixture.

- **`examples/python/06_spark_mapInPandas.py`** (~90 lines) —
  builds a tiny EMIR-shaped Spark DataFrame → native
  partition-friendly scan via `opendqi.spark.scan_spark_
  dataframe` → groupBy check_id. Skip-friendly on dev machines
  without Java/JDK installed (SparkSession startup wrapped in
  try/except).

### Changed

- **`opendqi.spark.scan_spark_dataframe(df, *, regime, mapping,
  normalize=False)`** — full rewrite. v0.13 went via
  `df.toPandas()` which collected the entire DataFrame to the
  driver (lost the distribution). v0.14 uses Spark's native
  `DataFrame.mapInPandas` so each partition is scanned
  independently and the issues stream back as a Spark
  DataFrame of the v1.0 stable 11-column issues schema. The
  `FutureWarning` is gone. The `EXPERIMENTAL` docstring marker
  is gone. Return type is now a `pyspark.sql.DataFrame` (was
  `pandas.DataFrame` — **breaking** for the v0.13 callers,
  acceptable because v0.13 was advertised experimental).

- **`docs/python.md`** — Status header bumped to v0.14.x,
  Install section gains the 3 extras, Polars section split
  into "ad-hoc analysis of `result.issues`" + new "scan a
  LazyFrame directly with push-down" subsections, Spark
  section rewritten around mapInPandas, API surface table
  gains a Data-platform v0.14 category.

- **`README.md`** — Install snippets gain the 3 extras, a 3rd
  Python snippet showcases `opendqi.spark.scan_spark_dataframe`,
  catalog paragraph names the 13 Python entry points (was 10),
  install tag references bumped v0.12.2 → v0.13.0 in the
  curl installer + `cargo install --tag` lines.

### Removed

- `FutureWarning` formerly emitted by
  `opendqi.spark.scan_spark_dataframe` on every call.
- `EXPERIMENTAL` marker from `opendqi.spark`'s module docstring
  and from `docs/python.md` Status & limitations.

## [0.13.0] - 2026-05-21

Python feature expansion — multi-file scans + cross-message
workflows + experimental Spark interop. **10 new Python entry
points** on top of the v0.12.0 trio (`scan_parquet`,
`scan_table`, `parse_xml`), bringing the Python surface to
feature parity with the high-value CLI subcommands.

The CTO-priority `opendqi.emir.tr_audit(tar=…, tsr=…,
feedback=…)` is now native — 3 files, 1 call, all 3 layers'
checks plus the 3 cross-layer `EMIR.AUD.*` coherence checks.

**ZERO Rust core change.** 216 checks remain 216 (151 EMIR +
65 SFTR). The v1.0 stable Arrow contract for `result.issues`
(locked v0.12.0 P3) is unchanged. 19/19 goldens byte-identical
with `UPDATE_GOLDEN` unset. 762/0 workspace tests pass. 59
pytest cases (30 v0.12 + 29 new v0.13) all green.

### Added

- **`opendqi.{emir,sftr}.scan_directory(path, *, normalize=False)`
  and `scan_files(paths, *, normalize=False)`** — multi-file
  aggregator entry points. `scan_directory` reuses
  `opendqi_io::discover_emir_inputs` (filters `.xml`/`.parquet`,
  expands `.zip`/`.gz` no zip-slip, sorted output, non-recursive
  `max_depth=1`); `scan_files` takes an explicit list. Both
  dispatch each path by extension to the right reader,
  aggregate records, and run the standard `default_*_checks()`
  suite. **`.csv` is rejected** in v0.13 (CSV needs a mapping;
  workaround documented in the error message). `summary.files_
  processed` reflects the actual number of files contributed
  records.

- **`opendqi.{emir,sftr}.tr_audit(*, tar, tsr, [feedback])`**
  — the CTO-priority cross-message workflow. EMIR variant takes
  3 paths (TAR `auth.030`, TSR `auth.107`, feedback `auth.092`),
  runs `default_checks` + `default_tr_state_checks` +
  `default_feedback_checks` per layer, then the 3 cross-layer
  `EMIR.AUD.*` (`compute_tr_audit_emir_issues`). SFTR variant
  takes 2 paths (no feedback layer — `SFTR.FBK.*` was retired
  in M0.4) + 2 cross-layer `SFTR.AUD.*`. All issues merged into
  one `PyScanResult` with `summary.files_processed = 3` (EMIR)
  or `2` (SFTR). Keyword-only args enforce explicit naming.

- **`opendqi.emir.collateral_audit(*, tsr, msr)`** — EMIR
  Article 11 collateral obligation check. Reads `auth.107` +
  `auth.109`, calls `compute_collateral_emir_issues` →
  `EMIR.COL.MISSING` (no MSR link, or all IM/VM amounts zero)
  + `EMIR.COL.STALE` (linked snapshot older than
  `collateral_max_age_days`). Mirror of the v0.10.0 CLI
  `emir collateral-audit`. Defaults thresholds (custom
  thresholds remain CLI-only via `--config`).

- **`opendqi.sftr.missing_collateral(auth083, *, tsr=None)`** —
  parses `auth.083` (Missing Collateral Request) + runs the 2
  base `SFTR.MCR.*` checks. If `tsr` is provided, also reads
  `auth.079` and runs the 3 cross-ref checks
  (`COLLATERAL_PRESENT_IN_TSR`, `STILL_MISSING_IN_TSR`,
  `REQUESTED_UTI_NOT_IN_TSR`). Mirror of the CLI's
  `sftr missing-collateral --tsr ...`. The `--store` CLI flag
  (looks up persisted SFTR state by UTI) is NOT wrapped in
  v0.13 — Python doesn't touch the SQLite store yet.

- **`opendqi.{emir,sftr}.book_reconcile(book, tsr, *, mapping=None)`**
  — internal book ↔ TR state reconciliation. **Dual book
  input**: `book` can be a `str` path (`.csv` requires
  `mapping`, `.parquet` doesn't — the parquet schema is
  canonical) OR a `pyarrow.Table` / `pyarrow.RecordBatch`
  already in memory (always requires `mapping`). `mapping`
  direction is the familiar `canonical_field →
  user_column_name`. Fires the 5 `EMIR.BREC.*` / `SFTR.BREC.*`
  checks (NOTIONAL/LOAN/COLLATERAL/VALUATION_MISMATCH,
  currency mismatches, MATURITY_MISMATCH, STATUS_MISMATCH).

- **`opendqi.spark.scan_spark_dataframe(df, *, regime, mapping,
  normalize=False)`** — **EXPERIMENTAL** pure-Python helper for
  Spark interop. Round-trips a PySpark DataFrame through
  pandas → Arrow → `scan_table` → pandas → Spark, returning a
  Spark DataFrame of the v1.0 stable 11-column issues table.
  **No PySpark dependency declared** in the wheel (duck-typed
  import inside the helper) — users `pip install pyspark`
  themselves. Emits a `FutureWarning` on every call to make the
  preview status visible. The native `mapInPandas` UDF version
  (partition-friendly, zero-copy) is on the v0.14 roadmap.

- **`examples/python/04_tr_audit.py`** — runnable demo of the
  new EMIR tr_audit workflow on the `quickstart-emir` 3-file
  kit. Prints summary, top-5 check_id frequency, EMIR.AUD.*
  cross-layer filter (showing what only tr_audit can produce).

### Changed

- **`crates/opendqi-py` refactored to maturin `python-source`
  layout** (commit `1bbae97`, structurally required for the
  pure-Python `opendqi/spark.py` to live alongside the compiled
  cdylib). The compiled lib was renamed `opendqi` → `_opendqi`
  (`Cargo.toml` `[lib] name = "_opendqi"`, `src/lib.rs`
  `#[pymodule] fn _opendqi`); a new `python/opendqi/__init__.py`
  re-exports the compiled extension's surface via `from
  ._opendqi import *`. `pyproject.toml` gained
  `python-source = "python"` + `module-name = "opendqi._opendqi"`.
  A `py.typed` PEP 561 marker also ships. **Zero functional
  change visible to users**: `import opendqi; opendqi.emir.
  scan_parquet(...)` keeps working identically. Wheel layout:
  v0.12.x had `opendqi.<arch>.so` at the wheel root; v0.13.0+
  has the `opendqi/` package containing `__init__.py`,
  `_opendqi.<arch>.so`, `py.typed`, and the new `spark.py`.
  Users who did `import opendqi._opendqi` directly (improbable)
  would break.

- **`docs/python.md`** restructured for v0.13: status header
  bumped to v0.13.x, NEW "Multi-file scans" and "Cross-message
  workflows" sections, API surface table restructured by
  category, Spark section rewritten to point at the new
  `opendqi.spark.scan_spark_dataframe` helper.

- **`README.md`** install block: keeps the 5-line minimal
  Python snippet, adds a second snippet showing
  `opendqi.emir.tr_audit(...)`. One sentence below names all
  10 new functions + `opendqi.spark`.

- **`examples/python/README.md`**: `04_tr_audit.py` promoted as
  the v0.13 highlight in the script table.

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
  `opendqi` in debug `--jobs 4` only when needed (project
  build-hygiene rule — never release in the dev loop). Runs the 3
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

[Unreleased]: https://github.com/PauFou/OpenDQI/compare/v0.13.0...HEAD
[0.13.0]: https://github.com/PauFou/OpenDQI/compare/v0.12.3...v0.13.0
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
