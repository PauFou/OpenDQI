# EMIR Reconciliation Statistics (auth.091)

`opendqi emir recon-stats <auth091.xml>` ingests an ISO 20022
`auth.091` message — the Trade Repository's statistical view of
pairing and reconciliation outcomes for the firm's submissions, by
counterparty and reporting period.

This layer is **statistical**: each record summarises rates and
counts for one counterparty over one period. It is distinct from
`auth.106` (per-trade reconciliation) and from `auth.092` (per-UTI
rejection feedback). See [`auth-messages.md`](auth-messages.md) for
the broader catalog.

## Command

```bash
opendqi emir recon-stats path/to/auth091.xml \
  --out ./recon-stats-report
```

Optional flags:

- `--prior path/to/previous_auth091.xml` — enables the trend check
  (`EMIR.RST.PAIRING_RATE_TREND_DOWN`). When omitted, only the three
  threshold-based checks fire.
- `--config thresholds.yml` — overrides default thresholds; see
  the `recon_stats` block of `Thresholds` (`crates/opendqi-core/src/config.rs`).

Outputs (in `--out`):

- `summary.json` — aggregated counts + quality score.
- `recon_stats_issues.csv` — all issues, deterministically ordered.
- `recon_stats_report.html` — HTML render.

## Checks (4)

| Check ID | Severity | Dimension | Description |
|---|---|---|---|
| `EMIR.RST.PAIRING_RATE_LOW` | high | consistency | Pairing rate < `recon_stats.pairing_rate_min` (default **0.85**). One issue per counterparty falling below the floor. |
| `EMIR.RST.RECON_RATE_LOW` | high | consistency | Reconciliation rate < `recon_stats.recon_rate_min` (default **0.70**). Indicates field-level disagreement with the counterparty's submission. |
| `EMIR.RST.OUTSTANDING_UNPAIRED_HIGH` | warning | consistency | Outstanding-unpaired count > `recon_stats.outstanding_unpaired_max` (default **1000**). Surfaces back-book hygiene problems. |
| `EMIR.RST.PAIRING_RATE_TREND_DOWN` | warning | consistency | Pairing rate dropped ≥ 5 percentage points versus the prior batch (per counterparty). Only fires when `--prior` is provided. |

## Configuring thresholds

```yaml
# thresholds.yml
recon_stats:
  pairing_rate_min: 0.90      # tighter than default
  recon_rate_min: 0.80
  outstanding_unpaired_max: 500
```

```bash
opendqi emir recon-stats auth091.xml --config thresholds.yml --out report/
```

## Synthetic schema

The official ESMA XSD is not redistributed. The parser
(`crates/opendqi-xml/src/emir_recon_stats.rs`) recognises a
plausible structure: a `<RcncltnSttstcsRpt>` root containing one
`<Hdr>` and N `<ReconStat>` leaves carrying `<RptgDt>`,
`<CtrPty><LEI>`, `<PairgRate>`, `<RcncltnRate>`,
`<OutsdngPaired>`, `<OutsdngUnpaired>`. When the real schema is
available, edit the leaf table in `commit_leaf` to align.

The bundled fixture lives in
[`examples/emir/recon_stats/auth091-sample.xml`](../examples/emir/recon_stats/auth091-sample.xml)
and triggers exactly one issue per `EMIR.RST.*` check.

## Why this matters

Pairing and reconciliation rates are leading indicators of
back-office data-quality drift. A counterparty whose pairing rate
collapses from 0.95 to 0.70 is signalling either an internal
matching change or a UTI scheme divergence — both warrant escalation
before they accumulate weeks of unpaired trades. OpenDQI surfaces
these as actionable issues alongside per-trade rejections so the
firm gets one consolidated DQ view across statistical and
per-trade TR feedback layers.
