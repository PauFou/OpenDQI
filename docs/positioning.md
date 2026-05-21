# OpenDQI positioning & roadmap

## Product one-liner

> OpenDQI turns EMIR/SFTR Trade Repository activity, state, and rejection files into actionable data quality intelligence.

## The three layers of a TR-firm conversation

A firm and its Trade Repository exchange three logically distinct
streams of information. OpenDQI is organised around these three
layers and keeps their reports separate.

### 1. Activity layer — TAR

The activity layer answers **"what was submitted or processed during
a period?"**.

- Input today: firm-side submissions, ingested by `opendqi emir scan`
  (CSV with mapping, or ISO 20022 `auth.030.001.03` XML).
- Input planned (Phase 2): the TR's `auth.030` activity returns,
  ingested by `opendqi emir tr-activity-scan`. This will surface
  action-type distributions, event-type distributions, repeated
  corrections, duplicate NEWT detection, and rejected-then-accepted
  analysis when combined with a feedback file.

The activity layer is **append-only** by nature — it describes
events. The OpenDQI history store persists every record so that
cross-batch lifecycle checks (MODI/ETRM without a prior NEWT,
duplicate NEWT) can run.

### 2. State layer — TSR

The state layer answers **"what does the TR currently believe is
outstanding?"**.

- Input today: not yet supported. Phase 1 (next milestone) will add
  `opendqi emir tr-state-scan <auth.107>` over the official Trade
  State Report, producing a dedicated `tr_state_report.html` with
  state-health checks: missing valuation on TSR, stale valuation
  on TSR, active trade past maturity, placeholder maturity date,
  duplicate active UTI, valuation after termination.

The state layer is **snapshot-oriented**. Comparing two consecutive
TSR snapshots (planned) reveals trades that have appeared, vanished,
or changed state at the TR.

### 3. Rejection layer

The rejection layer answers **"what failed, and why?"**.

- Input today: `opendqi emir feedback <auth.092>` (EMIR-only — SFTR
  has no rejection-feedback message). Feedback rows are persisted in
  the SQLite history store with an `open / resolved / stale` status
  driven by the top-level `opendqi feedback list/resolve/stale`
  workflow.
- Phase 3 will deepen this layer with top rejection causes, repeated
  rejected UTIs, ageing analytics, and (when a TAR is available)
  rejected-then-accepted detection.

The rejection layer is **finite**: every row should drive an
operational action (re-submission, root-cause fix, or marked as
stale).

## Why the layered approach matters

Mixing TAR, TSR, and rejection signals in a single "issue list"
is the operational anti-pattern that OpenDQI is built to avoid.
Each layer has a different remediation owner, a different cadence
(intraday / weekly / on-receipt), and a different statistical
profile. OpenDQI surfaces each layer as its own report
(`report.html`, `tr_state_report.html`, the rejection workflow), and
defers the cross-layer audit to a single explicit `tr-audit` command
(Phase 4).

## Roadmap

```
Phase 0 — stabilize current engine, audit auth.* naming, pivot positioning
Phase 1 — EMIR TSR health (`tr-state-scan` over `auth.107`)
Phase 2 — EMIR TAR activity intelligence (`tr-activity-scan` over `auth.030` TR-output mode)
Phase 3 — Rejection analytics (deepened `auth.092`)
Phase 4 — Combined `opendqi emir tr-audit` consolidating TAR + TSR + rejections
Phase 5 — Book vs TSR reconciliation
Phase 6 — SFTR equivalent modules
Phase 7 — Local web UI
```

Email notifications and ZIP/GZIP archive ingestion were cross-cutting
concerns delivered after the main layered roadmap; both are now
shipped. The web UI (`opendqi desktop`) is deliberately last so that
the CLI, parsers, report generators, and history store are stable
before any graphical layer is added.

## What OpenDQI is not

- **Not a Trade Repository, ARM, or reporting gateway.** OpenDQI
  does not submit reports. It analyses files before or after
  submission.
- **Not a regulatory certification tool.** Its checks are aligned
  with public ESMA Validation Rules where possible, but compliance
  remains the firm's responsibility.
- **Not a re-implementation of supervisory tooling.** It is a local
  engine for the firm's own internal controls.
- **Not a cloud service.** Local-first by default; no network calls,
  no telemetry, no managed backend.

### What a DQI is — and what it is NOT (v0.15+)

The Data Quality Pack adds aggregated indicators above the
216 granular checks. v0.15 shipped 10 EMIR indicators ;
v0.16 grew the catalogue to **28 indicators across both
régimes** (24 EMIR + 4 SFTR T2-layer) and switched the two
stale-data indicators to **TARGET2 business days**. Each
indicator ships with a `status` (`green` / `amber` / `red` /
`not_applicable`). The vocabulary discipline matters :

- **A DQI is an internal data quality indicator** — a
  numerator / denominator / rate triplet bucketed against a
  firm-configurable amber/red threshold.
- **A DQI is NOT a validation rule.** Validation rules are
  the row-level `EMIR.*` / `SFTR.*` checks that flag
  individual defects ; a DQI rolls them up.
- **A DQI is NOT a verdict of non-conformity.** A `red`
  status is an internal alert, not a regulatory declaration.
- **OpenDQI computes internal data quality indicators. It
  does not certify regulatory compliance.**

See [`data-quality-pack.md`](data-quality-pack.md#disclaimer--what-a-dqi-is-not) for the full disclaimer that we
recommend printing on any executive report cover page that
reuses the DQI pack output.
