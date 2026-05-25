# EMIR `auth.090` — Derivatives Trade Position Set Report

Per-message coverage note. Parent catalog: [`../auth-messages.md`](../auth-messages.md).
Command: `opendqi emir data-quality-pack --positions <auth090.xml> [...]`.

## Business meaning

`auth.090` is the trade repository's **aggregated-exposures
report** between a pair of counterparties. Each `Rpt` element
carries the shared reference date plus up to 500 000 records
across 4 position-set kinds :

- `PosSet` — outstanding-derivatives aggregate per (asset class,
  contract type, currency, …)
- `CcyPosSet` — same shape bucketed by currency
- `CollPosSet` — collateral position sets (collateral as the
  metric instead of notional)
- `CcyCollPosSet` — currency-bucketed collateral

This is the **largest XSD we parse** (~5 400 lines), with rich
buyer/seller + total/clean splits and per-leg notional /
direction / metric blocks. v0.18 ships an honest subset
covering the headline DQ-actionable metrics; the full per-leg
richness lives in `raw_fields`.

## XSD envelope

```text
Document (urn:iso:std:iso:20022:tech:xsd:auth.090.001.02)
└─ DerivsTradPosSetRpt
   └─ AggtdPos (PositionSetAggregated2Choice__1)
      ├─ DataSetActn = "NOTX"
      └─ Rpt (PositionSetAggregated4__1)
         ├─ RefDt (ISODate, shared across the file)
         ├─ PosSet[]        (up to 500_000)
         ├─ CcyPosSet[]
         ├─ CollPosSet[]
         └─ CcyCollPosSet[]
```

Each position-set wrapper has `Dmnsns + Mtrcs` :

- `Dmnsns` = the grouping tuple (CtrPty + AsstClss + CtrctTp +
  ValCcy + UndrlygInstrm + Coll + ...)
- `Mtrcs` = the aggregated values (Buyr/Sellr × Ttl/Clean ×
  NbOfTrds/PostvVal/NegVal/Ntnl for PosSet kinds; NbOfRpts +
  PstdMrgnOrColl + RcvdMrgnOrColl for CollPosSet kinds)

v0.18 extracts (per record) :

- `record_id` synthesised from `<source>#<kind>-<index>`
- `reference_date` shared across the file
- `position_set_kind` from the 4-way wrapper element name
- `reporting_counterparty` + `other_counterparty` (LEI)
- `asset_class` (`ProductType4Code`)
- `contract_type` (`FinancialInstrumentContractType2Code`)
- `value_currency` (ISO 4217)
- `underlying_id` (ISIN when reported)
- `notional` (first-wins on `Ttl/Buyr/Ntnl/Amt`)
- `mark_to_market_value` (first-wins on `Ttl/Buyr/PostvVal`)
- `collateral_value` (first-wins on `Ttl/PstdMrgnOrColl/.../Amt`)

## DQI coverage (4 DQIs, v0.18 E4)

| Indicator | Dimension | Rationale |
|---|---|---|
| `DQI_POSITION_NOTIONAL_MISSING` | completeness | share of PosSet/CcyPosSet records reporting no notional |
| `DQI_POSITION_MARK_TO_MARKET_MISSING` | completeness | share of PosSet/CcyPosSet records reporting no MtM |
| `DQI_POSITION_NOTIONAL_NEGATIVE` | accuracy | share of PosSet/CcyPosSet records with strictly-negative notional (structural defect) |
| `DQI_POSITION_COLLATERAL_MISSING` | completeness | share of CollPosSet/CcyCollPosSet records reporting no collateral amount |

Each DQI scopes by `position_set_kind` to avoid counting
records that structurally can't carry the field.

## Granular checks (4 `EMIR.POS.*`, v0.18 E5)

| Check ID | Dimension | Severity |
|---|---|---|
| `EMIR.POS.POSITION_SET_KIND_INVALID` | validity | critical |
| `EMIR.POS.NOTIONAL_NEGATIVE` | accuracy | critical |
| `EMIR.POS.ASSET_CLASS_ENUM_INVALID` | validity | high |
| `EMIR.POS.UNDERLYING_ID_MISSING` | completeness | high |

`UNDERLYING_ID_MISSING` is a honest rename from the plan's
`JURISDICTION_MISSING` — auth.090 has no jurisdiction field
at the XSD level; the most actionable completeness signal is
`UndrlygInstrm/.../ISIN` (the underlying identifier). The
check fires only on PosSet/CcyPosSet records with `notional > 0`
(collateral kinds aggregate across underlyings, and zero-notional
records carry no DQ signal).

## Scope notes (v0.18)

- v0.18 captures headline-Decimal-per-metric only. The rich
  Buyr/Sellr + Ttl/Clean splits live in `raw_fields`.
- No per-leg direction extraction (Direction2 / Direction4Choice
  subtree → raw_fields).
- No commodity sub-product extraction (EnergyCommodityCoal2
  et al. → raw_fields).
- Standalone `opendqi emir position-scan` subcommand
  intentionally not shipped — the `--positions` flag on
  `data-quality-pack` covers the same surface (consistent with
  how every other v0.18 SFTR layer ships, no standalone scan
  subcommand per layer).
