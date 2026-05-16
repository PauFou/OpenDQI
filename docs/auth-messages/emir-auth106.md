# EMIR `auth.106` — Derivatives Trade Warnings Report

Per-message coverage note. Parent catalog: [`../auth-messages.md`](../auth-messages.md).
Checks reference: [`../emir-warnings.md`](../emir-warnings.md).

## Business meaning

`auth.106` (`DerivativesTradeWarningsReportV01`, ESMA tag **DATWRN**)
is the trade repository's periodic **data-quality warnings**
statistics report: per reference date, how many outstanding
derivatives have **no / outdated valuation**, how many have **no /
outdated margin information**, and how many reported derivatives carry
an **abnormal (outlier) notional**. OpenDQI uses it to flag
report-level rates above configurable thresholds
(`opendqi emir warnings`, `EMIR.WRN.*`).

It is **not** a counterparty pairing / reconciliation report — EMIR
has no such message. (The earlier synthetic "pairing" shape that was
mislabelled `auth.106` is removed; see
[`../auth-messages.md`](../auth-messages.md).)

## Direction

**TR → firm.**

## Coverage status

**schema-verified (subset, derived).** The parser parses the real
`auth.106.001.01` envelope, but the message carries **no rate
fields** — OpenDQI **derives** the missing/outdated/abnormal rates
from the report-level counts and projects them onto the scalar
`TradeWarningsRecord`. The derivation is an explicit, documented
interpretation, not a verbatim field mapping.

## Real envelope

```
Document
└─ DerivsTradWrnngsRpt  (DerivativesTradeWarningsReportV01)
   └─ WrnngsSttstcs  (StatisticsPerCounterparty16Choice — choice)
      ├─ DataSetActn = "NOTX"            (no-activity / empty report)
      └─ Rpt  (DetailedStatisticsPerCounterparty17)
         ├─ RefDt  (ISODate)
         ├─ MssngValtn  (choice: DataSetActn | Rpt):
         │     NbOfOutsdngDerivs, NbOfOutsdngDerivsWthNoValtn,
         │     NbOfOutsdngDerivsWthOutdtdValtn
         │     [+ Wrnngs(0..500000): CtrPtyId, …counts, TxDtls — deferred]
         ├─ MssngMrgnInf (choice: DataSetActn | Rpt):
         │     NbOfOutsdngDerivs, NbOfOutsdngDerivsWthNoMrgnInf,
         │     NbOfOutsdngDerivsWthOutdtdMrgnInf  [+ Wrnngs — deferred]
         └─ AbnrmlVals  (choice: DataSetActn | Rpt):
               NbOfDerivsRptd, NbOfDerivsRptdWthOtlrs  [+ Wrnngs — deferred]
```

Accepted root namespace:
`urn:iso:std:iso:20022:tech:xsd:auth.106.001.01`.

## Derivation → canonical `TradeWarningsRecord`

One record per `WrnngsSttstcs/Rpt` (per `RefDt`); the report-level
category `Rpt` counts only (the per-counterparty `Wrnngs` breakdown is
**not** read — `counterparty_lei` stays `None`):

| Canonical field | Source |
|---|---|
| `reporting_date` | `WrnngsSttstcs/Rpt/RefDt` |
| `outstanding_derivatives` / `missing_valuation` / `outdated_valuation` | `MssngValtn/Rpt/{NbOfOutsdngDerivs, …WthNoValtn, …WthOutdtdValtn}` |
| `outstanding_derivatives_margin` / `missing_margin_info` / `outdated_margin_info` | `MssngMrgnInf/Rpt/{NbOfOutsdngDerivs, …WthNoMrgnInf, …WthOutdtdMrgnInf}` |
| `derivatives_reported` / `abnormal_values` | `AbnrmlVals/Rpt/{NbOfDerivsRptd, NbOfDerivsRptdWthOtlrs}` |
| `missing_valuation_rate` | `missing_valuation / outstanding_derivatives` (else `None`) |
| `outdated_valuation_rate` | `outdated_valuation / outstanding_derivatives` |
| `missing_margin_rate` | `missing_margin_info / outstanding_derivatives_margin` |
| `outdated_margin_rate` | `outdated_margin_info / outstanding_derivatives_margin` |
| `abnormal_values_rate` | `abnormal_values / derivatives_reported` |
| `regime` | `Emir` |

`WrnngsSttstcs/DataSetActn = NOTX` → zero records + Info
`EMIR.FMT.WRN_NO_RECORDS`.

## Fields ignored / known unsupported branches

The per-counterparty **`Wrnngs`** lists
(`MissingValuationsData2` / `MissingMarginData2` /
`AbnormalValuesData4`, 0..500000 each — `CtrPtyId`, per-counterparty
counts, and the per-UTI `TxDtls`) and the per-category `DataSetActn`
no-activity branches.

### Documented limitations

- **Derived rates.** The real message has no rate field; the rates
  are computed from the report-level counts (a defensible
  interpretation, not a verbatim mapping).
- **Report-level only.** The per-counterparty `Wrnngs` breakdown
  (with LEIs and per-UTI `TxDtls`) is a deferred subset →
  `counterparty_lei` is `None` and the `EMIR.WRN.*` checks operate on
  the report aggregate. A per-counterparty / per-UTI model is a
  separate future increment (same precedent as the `auth.091`
  per-transaction detail).
- **Single reference date assumption.** One record per `Rpt`; the
  schema permits one `Rpt` per report.
- **Not a full XSD validation** — same documented "subset" stance as
  `auth.107`; use `--xsd` with a locally-held official schema for
  strict validation (see [`../xsd-validation.md`](../xsd-validation.md)).

## Schema source used

ESMA EMIR REFIT outgoing-messages usage guideline
**`auth.106.001.01_ESMAUG_DATWRN_1.1.0`** (base message
`auth.106.001.01`, `DerivativesTradeWarningsReportV01`). The
SWIFT-licensed XSD is held **locally only** (`ESMA_docs/`, gitignored)
and is **never** redistributed or excerpted; only element names,
nesting and cardinalities were used to align the parser.

## Verification procedure

1. `cargo test -p opendqi-xml --lib emir_warnings`
2. `cargo test -p opendqi-xml --test warnings_integration` (derived
   rates, high-rate checks, no-activity path).
3. `opendqi emir warnings examples/emir/warnings/auth106-sample.xml
   --out /tmp/wrn` → `EMIR.WRN.MISSING_VALUATION_HIGH` /
   `EMIR.WRN.MISSING_MARGIN_INFO_HIGH` /
   `EMIR.WRN.ABNORMAL_VALUES_HIGH`;
   `…/auth106-no-records.xml` → zero records + one
   `EMIR.FMT.WRN_NO_RECORDS` info note, no error.
4. Optional strict check against a locally-held official XSD (not
   committed): `xmllint --noout --schema <local auth.106 xsd> <file>`
   (fixtures are schema-shaped subsets, so a full pass is not
   asserted).
