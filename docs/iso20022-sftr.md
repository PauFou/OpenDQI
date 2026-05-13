# ISO 20022 auth.052 (SFTR) support

OpenDQI reads SFTR (Securities Financing Transactions Regulation)
reports in the official ISO 20022 format `auth.052.001.02`
(`SecuritiesFinancingReportingTransactionReport`).

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
[`xsd-validation.md`](xsd-validation.md) infrastructure (planned
`opendqi sftr scan --xsd <path>` — currently the `--xsd` flag is
implemented for `emir scan`; SFTR validation via xmllint will land in
a follow-up).

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
