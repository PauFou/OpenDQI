# EMIR Article 11 — risk-mitigation checks for non-cleared OTC

EMIR Article 11 imposes several risk-mitigation duties on counterparties
to a non-centrally-cleared OTC derivative: timely confirmation, portfolio
reconciliation, dispute resolution, compression, daily valuation, and
the exchange of variation / initial margin.

OpenDQI ships 10 `EMIR.RMT.*` checks that look at the canonical
`EmirRecord` fields and surface gaps in those duties. Every check
filters on `clearing_status` first and only runs when the trade is
**non-cleared** — accepted shorthand: `NCLR`, `NCMP`, `non-cleared`,
`uncleared`, `false`, `n`, `no`.

These checks live in `default_checks()` alongside the rest of the
single-batch EMIR catalog; they fire automatically when `opendqi emir
scan` runs.

## Catalog

| Check ID | Dim | Severity | Trigger |
|---|---|---|---|
| `EMIR.RMT.UNCLEARED_NEEDS_CONFIRMATION` | Completeness | High | Non-cleared trade with no `confirmation_method`. |
| `EMIR.RMT.LATE_CONFIRMATION` | Timeliness | High | Reporting > 1 business day after execution for FC (> 2 for NFC). |
| `EMIR.RMT.PORTFOLIO_RECONCILIATION_MISSING` | Completeness | Warning | Margin posted/collected but no `collateral_portfolio_code`. |
| `EMIR.RMT.DAILY_VALUATION_MISSING` | Timeliness | High | Outstanding non-cleared trade with `valuation_timestamp` missing or > 1 day old. |
| `EMIR.RMT.VARIATION_MARGIN_MISSING` | Completeness | High | Outstanding non-cleared trade with no VM posted or collected. |
| `EMIR.RMT.INITIAL_MARGIN_THRESHOLD` | Completeness | Warning | Non-cleared notional above the configured AANA threshold but no IM posted or collected. Default: 8 G€ (phase 6). |
| `EMIR.RMT.COLLATERAL_CATEGORY_REQUIRED` | Completeness | Warning | Non-cleared trade carries margin but no `collateralisation_category`. |
| `EMIR.RMT.NFC_ABOVE_CLEARING_THRESHOLD` | Accuracy | Warning | NFC counterparty trading uncleared above its asset-class-specific clearing threshold — verify Article 10. Defaults: IR/FX 3 G€, CR/EQ 1 G€, CO 4 G€. |
| `EMIR.RMT.INTRAGROUP_NEEDS_INDICATOR` | Completeness | Warning | Reporting entity = a counterparty but `intragroup_indicator` absent. |
| `EMIR.RMT.MASTER_AGREEMENT_REQUIRED` | Completeness | Warning | Non-cleared trade with no `master_agreement_type`. |

## Thresholds (configurable)

Both thresholds live under `emir_rmt:` in the `Thresholds` YAML
config and can be overridden via `--config`. Defaults reflect the
current ESMA Article 10 / RTS values:

| Asset class | NFC clearing threshold (default) |
|---|---|
| Credit (`CR`) | 1 G€ |
| Equity (`EQ`) | 1 G€ |
| Interest rate (`IR`) | 3 G€ |
| FX (`FX`) | 3 G€ |
| Commodity (`CO`) | 4 G€ |

AANA initial-margin threshold: **8 G€** (phase 6).

`NFC_ABOVE_CLEARING_THRESHOLD` now covers all five asset classes
(`IR`, `CR`, `EQ`, `FX`, `CO`) — not only IR/CR as in v1.

The asset_class string on each record is canonicalised via
[`opendqi_core::dq::formats::canonical_asset_class`], which accepts
both short codes (`IR`, `CR`, …) and the ISO 20022 / ESMA aliases
(`INTR`, `CRDT`, `EQTY`, `CURR`, `COMM`). Records with an unknown
code are skipped by the check.

See [`examples/config/emir-rmt-thresholds.yml`](../examples/config/emir-rmt-thresholds.yml)
for a working example. YAML semantics:

- Omitting the `emir_rmt` section keeps every default.
- Omitting only `nfc_clearing_thresholds_eur` keeps the ESMA defaults
  for the map.
- Providing `nfc_clearing_thresholds_eur` **replaces the entire map**
  — the caller must list every class they want; classes absent from
  the YAML are no longer checked.
- `aana_im_threshold_eur` falls back to 8 G€ when omitted.

This is the right escape hatch when ESMA bumps a phase (e.g. raising
the AANA back to 50 G€) or when a firm wants to use bespoke internal
thresholds for triage.

## Design notes

- No new trait — these checks live on `EmirRecord` and reuse the
  existing `Check` trait + `default_checks()` registry. Each check
  filters on `is_uncleared(clearing_status)` at the top of its `run`.
  This is consistent with the tier-2 / tier-3 EMIR consistency checks
  that filter by `asset_class` or `nature`.
- All checks live in a single module
  (`crates/opendqi-core/src/dq/risk_mitigation.rs`) because they share
  the same filter helper.
- The helper `opendqi_core::dq::is_uncleared` is `pub` for re-use by
  future Article 11 work (e.g. portfolio reconciliation report
  ingestion).

## Fixture

`examples/emir/risk_mitigation/rmt_sample.csv` (+ matching
`rmt_mapping.yml`) holds 12 trades:

- Rows R001-R010 each break a specific Article 11 duty (one row per
  RMT check).
- Row R011 is clean — should fire nothing.
- Row R012 is cleared — should fire nothing (the filter must skip).

Run:

```bash
opendqi emir scan \
  examples/emir/risk_mitigation/rmt_sample.csv \
  --mapping examples/emir/risk_mitigation/rmt_mapping.yml \
  --out ./report-rmt/
grep 'EMIR.RMT' ./report-rmt/issues.csv | cut -d, -f1 | sort -u
```

All 10 `EMIR.RMT.*` check IDs should appear at least once.
