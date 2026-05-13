# ISO 20022 auth.030 (EMIR Refit) support

OpenDQI reads EMIR Refit reports in the official ISO 20022 format
`auth.030.001.03` (DerivativesTradeReport). The adapter is selected
**automatically** when the root namespace of the input XML is
`urn:iso:std:iso:20022:tech:xsd:auth.030.001.03` — there is no
flag to pass.

```bash
opendqi emir scan ./real-emir-auth030.xml --out ./report/
```

The same five MVP DQ checks (missing UTI, missing valuation, abnormal
maturity, duplicate UTI, late reporting) fire against the extracted
records.

## Where to obtain the XSD

OpenDQI does **not** redistribute the auth.030 XSD: it is a
SWIFT-licensed component that may not be embedded in third-party
distributions. Get it directly from ESMA:

- ESMA EMIR reporting hub: <https://www.esma.europa.eu/data-reporting/emir-reporting>
- SWIFT MyStandards (free account required): the
  "EMIR Refit – Incoming Messages" and "EMIR Refit – Outgoing
  Messages" bundles include the XSD, PDF guidelines, and a comparison
  workbook against the previous version.

Once you have the file locally, you can validate against it via the
existing `--xsd` flag (see [`xsd-validation.md`](xsd-validation.md)):

```bash
opendqi emir scan ./real-emir-auth030.xml \
  --xsd /local/path/auth.030.001.03_ESMAUG_DATTAR_1.1.0.xsd \
  --out ./report/
```

## Action-type mapping

The wrapping element under `<Rpt>` determines `EmirRecord.action_type`:

| auth.030 element | `action_type` code |
|---|---|
| `<New>` | `NEWT` |
| `<Mod>` | `MODI` |
| `<Crrctn>` | `CORR` |
| `<EarlyTermntn>` | `ETRM` |
| `<PosCmpnt>` | `POSC` |
| `<ValtnUpd>` | `VALU` |
| `<MrgnUpd>` | `MARU` |
| `<TradEx>` | `OTHR` |

## Typed-field mapping (selection)

Paths are relative to the action element (e.g. `<New>`, `<Mod>`, ...).

| `EmirRecord` field | auth.030 path |
|---|---|
| `uti` | `CmonTradData/TxData/TradId/UnqTxIdr` |
| `prior_uti` | `CmonTradData/TxData/TradId/PrrUnqTxIdr/UnqTxIdr` |
| `entity_responsible_for_reporting` | `CtrPtySpcfcData/NttyRspnsblForRpt/LEI` (falls back to `RptgCtrPty/Id/LEI`) |
| `counterparty_1` | `CtrPtySpcfcData/RptgCtrPty/Id/LEI` |
| `counterparty_2` | `CtrPtySpcfcData/OthrCtrPty/Id/LEI` or `Id/Org/LEI` or `Id/Org/AnyBIC` |
| `asset_class` | `CmonTradData/CtrctData/AsstClss` |
| `product_id` | `CmonTradData/CtrctData/PdctClssfctn` |
| `master_agreement_type` / `_version` | `CmonTradData/CtrctData/MstrAgrmt/Tp` / `Vrsn` |
| `event_type` | `CmonTradData/TxData/EvtTp` |
| `execution_timestamp` / `event_timestamp` / `reporting_timestamp` | `ExctnTmStmp` / `EvtTmStmp` / `RptgTmStmp` |
| `effective_date` / `maturity_date` / `termination_date` | `EffctvDt` / `MtrtyDt` / `TermntnDt` |
| `notional_amount` + `notional_currency` | `NtnlAmt/Lgs/FrstLgNtnl/Amt` + `@Ccy` |
| `leg2_notional_amount` + `_currency` | `NtnlAmt/Lgs/ScndLgNtnl/Amt` + `@Ccy` |
| `valuation_amount` + `_currency` + `_timestamp` + `_type` | `Valtn/Amt` + `@Ccy` + `Valtn/TmStmp` + `Valtn/Tp` |
| `clearing_status` | `Clr/Clrd` (`true` → `CLRD`, `false` → `NCLR`) |
| `clearing_ccp_lei` | `Clr/CCP/LEI` |
| `collateral_portfolio_code` / `collateralisation_category` | `Coll/PrtflCd` / `Coll/CollstnInd` |
| `intragroup_indicator` | `NtraGrpTx` |
| `hedging_indicator` | `DrctlyLnkdToCmrclActvtyOrTrsr` |
| `trading_capacity` / `nature` / `corporate_sector` | under `CtrPtySpcfcData/RptgCtrPty/...` |
| `initial_margin_posted` / `_collected` | `InitlMrgnPstd/Amt` / `InitlMrgnColl/Amt` (under `<MrgnUpd>`) |
| `variation_margin_posted` / `_collected` | `VartnMrgnPstd/Amt` / `VartnMrgnColl/Amt` |
| `delta` / `gamma` / `vega` | `Valtn/Dlt` / `Gmma` / `Vega` |
| `mtm_value_change` | `Valtn/Chng` or `Valtn/MtmChng` |

This list is illustrative; the source of truth is the match table in
`crates/opendqi-xml/src/emir/iso20022.rs`.

## `raw_fields` — exhaustive catch-all

Any element atteint by the parser but not in the typed-routing table
is stored in `EmirRecord.raw_fields`. The map key is the path
relative to the action element; the value is the leaf text,
optionally followed by `|Key=Value` pairs for significant
attributes:

```json
"raw_fields": {
  "PrtflCd": "PORT-001",
  "CtrPtySpcfcData/RptgCtrPty/AddtlData": "..."
}
```

This guarantees that **no information is lost** during ingestion.
Future DQ checks can read from `raw_fields` without modifying the
parser.

## Status and limitations

- Implemented against ESMA Usage Guideline `auth.030.001.03_ESMAUG_DATTAR_1.1.0`
  (EMIR Refit, in production since April 2024).
- The routing is namespace-only — the version segment of the
  namespace must be `auth.030.001.03`. New versions (e.g.
  `001.04`) will fall back to the simplified extractor until added
  here.
- SFTR (`auth.108.001.01`) is not yet supported. The architecture is
  ready to extend with a parallel adapter.
- Deeply nested commodity sub-classifications are captured into
  `raw_fields` but not promoted to typed fields.

## Synthetic fixture

A hand-authored sample lives under
`examples/emir/iso20022/sample.xml`. It contains zero real data and
covers the eight action types plus the same DQ patterns as the v0.1
simplified fixture.
