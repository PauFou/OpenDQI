# ISO 20022 SFTR support — 5 supported messages

OpenDQI parses 5 ISO 20022 messages for SFTR (Securities Financing
Transactions Regulation) reporting. The adapter dispatches by root
namespace ; firm-submission ingestion is auth.052, and the four
TR-output reports (TSR / reconciliation / missing-collateral /
MSR) each feed dedicated layers of the DQI pack.

| Message | Official ESMA name | Direction | OpenDQI consumer |
|---|---|---|---|
| `auth.052.001.02` | `SecuritiesFinancingReportingTransactionReport` | Firm → TR | `opendqi sftr scan`, `--tar` on `data-quality-pack` |
| `auth.079.001.02` | `SecuritiesFinancingReportingTransactionStateReport` | TR → firm / NCA | `opendqi sftr tr-state-scan`, `--tsr` on `data-quality-pack` |
| `auth.080.001.02` | `SecuritiesFinancingReportingReconciliationStatusAdvice` | TR → firm | `opendqi sftr reconcile`, `--reconciliation` on `data-quality-pack` |
| `auth.083.001.02` | `SecuritiesFinancingReportingMissingCollateralRequest` | TR → firm | `opendqi sftr missing-collateral`, `--missing-collateral` on `data-quality-pack` |
| `auth.085.001.02` | `SecuritiesFinancingReportingMarginDataTransactionStateReport` | TR → firm / NCA | `--msr` on `data-quality-pack` (v0.17+) — portfolio-level, CCP-cleared only |

Per-message reference pages with XSD path mapping, the canonical
projected record, and the DQIs/checks each one powers :

- [`auth-messages/sftr-auth079.md`](auth-messages/sftr-auth079.md) — TSR
- [`auth-messages/sftr-auth080.md`](auth-messages/sftr-auth080.md) — reconciliation
- [`auth-messages/sftr-auth083.md`](auth-messages/sftr-auth083.md) — missing-collateral
- [`auth-messages/sftr-auth085.md`](auth-messages/sftr-auth085.md) — MSR (new in v0.17)

The remainder of this page documents the **auth.052 firm-submission
adapter** in detail — the canonical entry point for `opendqi sftr
scan`. For the other 4 messages, follow the per-message page above.

## auth.052 — firm-submission adapter

```bash
opendqi sftr scan ./real-sftr-auth052.xml --out ./report/
```

The adapter is selected automatically when the root namespace is
`urn:iso:std:iso:20022:tech:xsd:auth.052.001.02`.

## Where to obtain the XSD

OpenDQI does **not** redistribute the auth.052 XSD: it is a
SWIFT-licensed component. Get it from:

- ESMA SFTR reporting hub: <https://www.esma.europa.eu/data-reporting/sftr-reporting>
- SWIFT MyStandards (free account required).

Use a locally-downloaded XSD with the existing
[`xsd-validation.md`](xsd-validation.md) infrastructure:
`opendqi sftr scan --xsd <path>` validates every XML input against the
schema and adds one `SFTR.FMT.XSD_VIOLATION` issue per error line.

## Action-type mapping

| auth.052 element | `action_type` code |
|---|---|
| `<New>` | `NEWT` |
| `<Mod>` | `MODI` |
| `<Crrctn>` | `CORR` |
| `<EarlyTermntn>` | `ETRM` |
| `<ValtnUpd>` | `VALU` |
| `<CollUpd>` | `COLU` |
| `<MrgnUpd>` | `MARU` |
| `<ReuseUpd>` | `REUU` |
| `<PosCmpnt>` | `POSC` |
| `<TradEx>` | `OTHR` |

## SFT-type mapping

The wrapper directly under `<LnData>` indicates the SFT type:

| Wrapper | `sft_type` code |
|---|---|
| `<Repo>` | `REPO` |
| `<BsbSctyTrad>` | `BSB` |
| `<SctyLndg>` | `SLEB` |
| `<MgnLndg>` | `MGLD` |

## Typed-field mapping (selection)

Paths are relative to the action element (`<New>`, `<Mod>`, etc.).
`<sft-type>` stands for any of the four wrappers above.

| `SftrRecord` field | auth.052 path |
|---|---|
| `uti` | `LnData/<sft-type>/UnqTxIdr` |
| `prior_uti` | `LnData/<sft-type>/PrrUnqTxIdr/UnqTxIdr` |
| `event_type` | `LnData/<sft-type>/EvtTp` |
| `execution_timestamp` | `LnData/<sft-type>/ExctnDtTm` |
| `event_timestamp` | `LnData/<sft-type>/EvtDtTm` (else falls back to `ExctnDtTm`) |
| `reporting_timestamp` | `LnData/<sft-type>/RptgDtTm` |
| `effective_date` | `LnData/<sft-type>/EvntDt` |
| `maturity_date` | `LnData/<sft-type>/MtrtyDt` |
| `termination_date` | `LnData/<sft-type>/TermntnDt` |
| `settlement_date` | `LnData/<sft-type>/SttlmDt` |
| `loan_value` + `loan_currency` | `LnData/<sft-type>/PrncplAmt/Amt` + `@Ccy` (or `LnVal/Amt`) |
| `master_agreement_type` / `_version` | `LnData/<sft-type>/MstrAgrmt/Tp` / `Vrsn` |
| `rebate_rate` | `LnData/Repo/RbtRate` |
| `lending_fee` | `LnData/SctyLndg/LndgFee` |
| `collateral_value` + `_currency` | `CollData/CollVal/Amt` + `@Ccy` |
| `haircut` | `CollData/Hrcut` |
| `reuse_indicator` | `CollData/AvlblForCollReuse` |
| `collateral_portfolio_code` | `CollData/PrtflCd` |
| `collateral_isin` | `CollData/Sctys/Id` |
| `entity_responsible_for_reporting` | `CtrPtySpcfcData/NttyRspnsblForRpt/LEI` (falls back to reporting counterparty LEI) |
| `counterparty_1` | `CtrPtySpcfcData/RptgCtrPty/Id/LEI` |
| `counterparty_2` | `CtrPtySpcfcData/OthrCtrPty/Id/LEI` / `Id/Org/LEI` / `Id/Org/AnyBIC` |
| **any other leaf** | `raw_fields[path]` |

See the source-of-truth match table in
`crates/opendqi-xml/src/sftr/iso20022.rs`.

## `raw_fields` — exhaustive catch-all

Same contract as the EMIR adapter: any leaf encountered but not
listed in the routing table lands in
`SftrRecord.raw_fields["<path>"]`. No data is lost; future DQ checks
can read from `raw_fields` without touching the parser.

## Synthetic fixture

A hand-authored sample lives at
`examples/sftr/iso20022/sample.xml`. It covers Repo, Buy-Sell-Back,
Securities Lending, plus the five MVP DQ patterns.

## Limitations

- Margin lending wrapper (`<MgnLndg>`) is recognised but its specific
  fields are not deeply mapped beyond the common subset.
- ValuationUpdate / CollateralUpdate / MarginUpdate / ReuseUpd
  records are emitted but their dedicated child paths beyond
  `CollData` are captured into `raw_fields` only.
- No CSV ingestion for SFTR yet — only the ISO 20022 XML format.
