# SFTR `auth.085` — Securities Financing Reporting Margin Data Transaction State Report (MSR)

Per-message coverage note. Parent catalog: [`../auth-messages.md`](../auth-messages.md).
Command: `opendqi sftr data-quality-pack --msr <auth085.xml> [...]`.

## Business meaning

`auth.085` is the trade repository's **portfolio-level state of
margins exchanged on CCP-cleared securities financing transactions**.
The TR sends it (or makes it available) to the firm / NCA / report-
submitting entity / entity responsible for reporting. Each `Stat`
record reports the latest IM / VM / excess-collateral amounts the TR
holds for one collateral portfolio, on both the **posted** side (the
reporting counterparty's contribution) and the **received** side
(what came from the other counterparty).

This is the SFTR analogue of the **EMIR `auth.109`** Margin State
Report — but with three structural differences :

1. **Portfolio-level, not trade-level.** `auth.085`'s per-record
   element type (`CollateralMarginNew10__1`) has no `UnqTradIdr` ;
   records are keyed by `CollPrtflId`. CCP-cleared margins are
   margined per portfolio, not per UTI.
2. **Restricted to CCP-cleared SFTs** per the ESMA scope statement
   on the message envelope (`SecuritiesFinancingReporting
   MarginDataTransactionStateReportV02`).
3. **6 amount fields, not 8.** SFTR does NOT split pre-haircut and
   post-haircut on margin amounts (EMIR auth.109 does). The 6
   amounts per portfolio are :
   - `InitlMrgnPstd` / `VartnMrgnPstd` / `XcssCollPstd` (posted)
   - `InitlMrgnRcvd` / `VartnMrgnRcvd` / `XcssCollRcvd` (received)
   
   `XcssColl*` is SFTR-specific — no EMIR auth.109 equivalent.

## Direction

**TR → competent authority / report submitting entity / reporting
counterparty / entity responsible for reporting.**

## Coverage status

**schema-verified (full payload).** The real `auth.085.001.02`
envelope is parsed
(`crates/opendqi-xml/src/sftr_margin_state.rs`,
`read_sftr_margin_state_xml`, namespace-dispatched against
`urn:iso:std:iso:20022:tech:xsd:auth.085.001.02`) and projected onto
the canonical [`SftrMarginStateRecord`]
(`crates/opendqi-core/src/model.rs`).

Real envelope :

```
Document
└─ SctiesFincgRptgMrgnDataTxStatRpt
   └─ TradData
      ├─ DataSetActn = "NOTX"        (no-activity report)
      └─ Stat   (1..n)               (CollateralMarginNew10__1)
         ├─ TechRcrdId                              (Max140Text)
         ├─ RptgDtTm                                (ISODateTime)
         ├─ EvtDt                                   (ISODate)
         ├─ CtrPty (Counterparty39__1)
         │  ├─ RptgCtrPty/<choice>/LEI              → reporting_counterparty
         │  └─ OthrCtrPty/<choice>/{Lgl/LEI | Ntrl/Id}  → other_counterparty
         ├─ CollPrtflId                             (Max52Text, mandatory)
         ├─ PstdMrgnOrColl  [0..1]
         │  ├─ InitlMrgnPstd/Amt(@Ccy)
         │  ├─ VartnMrgnPstd/Amt(@Ccy)
         │  └─ XcssCollPstd/Amt(@Ccy)
         ├─ RcvdMrgnOrColl  [0..1]
         │  ├─ InitlMrgnRcvd/Amt(@Ccy)
         │  ├─ VartnMrgnRcvd/Amt(@Ccy)
         │  └─ XcssCollRcvd/Amt(@Ccy)
         └─ CtrctMod/ActnTp                         → action_type
```

The 6 amounts share a single `margin_currency` promoted from the
first observed `@Ccy` attribute (first-wins ; the XSD allows them
to differ but in practice a portfolio has a single margining
currency, and the granular `SFTR.T3.MARGIN_CURRENCY_MISSING` check
flags missing currencies).

## Where to obtain the XSD

OpenDQI does **not** redistribute the auth.085 XSD : it is a
SWIFT-licensed component. Get it from :

- ESMA SFTR reporting hub : <https://www.esma.europa.eu/data-reporting/sftr-reporting>
  (the "Inter TR data exchange" bundle, March 2023 release).
- SWIFT MyStandards (free account required).

Validate every XML input against the schema via the existing
[`../xsd-validation.md`](../xsd-validation.md) infrastructure :
`opendqi sftr scan --xsd <path-to-xsd-dir>` adds one
`SFTR.FMT.XSD_VIOLATION` issue per error line.

## What OpenDQI uses it for

Two layers fire on the `--msr` input :

### 4 aggregate DQI computers (in `data-quality-pack`)

| Indicator | Dimension | Question |
|---|---|---|
| `DQI_T3_MARGIN_POSTED_MISSING_SFTR` | completeness | What share of portfolios report no posted margin at all ? |
| `DQI_T3_MARGIN_RECEIVED_MISSING_SFTR` | completeness | What share of portfolios report no received margin at all ? |
| `DQI_T3_EXCESS_COLLATERAL_USE_SFTR` | accuracy | What share of portfolios report XcssColl > 0 on either side ? (TR-side reporting inflation OR operational over-collateralisation signal) |
| `DQI_T3_MARGIN_STALE_SFTR` | timeliness | What share of portfolios have a `state_as_of` older than the TARGET2 business-day threshold ? |

See [`../data-quality-pack.md`](../data-quality-pack.md) for full
indicator details + threshold defaults.

### 6 granular per-record checks (in `issues.csv`)

| Check ID | Dimension | Severity | Trigger |
|---|---|---|---|
| `SFTR.T3.IM_POSTED_MISSING` | completeness | High | posted block has at least one other amount but `initial_margin_posted` is None |
| `SFTR.T3.VM_POSTED_MISSING` | completeness | High | symmetric on `variation_margin_posted` |
| `SFTR.T3.IM_RECEIVED_MISSING` | completeness | High | received side, IM |
| `SFTR.T3.VM_RECEIVED_MISSING` | completeness | High | received side, VM |
| `SFTR.T3.MARGIN_NEGATIVE` | accuracy | Critical | any of the 6 amounts is strictly < 0 |
| `SFTR.T3.MARGIN_CURRENCY_MISSING` | completeness | High | ≥ 1 amount populated but `margin_currency` is None |

The 4 _MISSING checks fire only on **partial-side reporting**
(some amounts set, the named field None). Fully-empty sides are
aggregated by the corresponding `DQI_T3_MARGIN_{POSTED,RECEIVED}_
MISSING_SFTR` DQI rather than per-record.

## Granular checks summary

| ID | Dimension | Severity |
|---|---|---|
| `SFTR.T3.IM_POSTED_MISSING` | completeness | High |
| `SFTR.T3.VM_POSTED_MISSING` | completeness | High |
| `SFTR.T3.IM_RECEIVED_MISSING` | completeness | High |
| `SFTR.T3.VM_RECEIVED_MISSING` | completeness | High |
| `SFTR.T3.MARGIN_NEGATIVE` | accuracy | Critical |
| `SFTR.T3.MARGIN_CURRENCY_MISSING` | completeness | High |

Registered in `default_sftr_msr_checks()` and dispatched
automatically by `compute_sftr_dqi_pack` when the `msr` input slot
is populated.

## Known limitations (v0.17)

- The `msr` slot accepts a **single** auth.085 file path ; multi-file
  directory aggregation is not yet wired (matches the rest of
  `data-quality-pack`'s single-path-per-layer contract).
- `pyarrow.Table` dual input on the SFTR side is scheduled for
  v0.18 — Python `opendqi.sftr.data_quality_pack(msr=...)` is
  **paths-only** today.
- Lifecycle tracking across MSR snapshots (e.g. "did this portfolio's
  margin drift > N % since yesterday ?") is not implemented in v0.17
  — would need an MSR history store similar to what `--store` does
  for TSR.
- No Parquet schema for `SftrMarginStateRecord` (consistent with
  EMIR `MarginStateRecord` which doesn't have one either) — MSR
  records flow through XML → in-memory slice → DQI computers, never
  Parquet on disk.
