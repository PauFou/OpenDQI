# SFTR `auth.070` — Securities Financing Reporting Transaction Margin Data Report (MAR)

Per-message coverage note. Parent catalog: [`../auth-messages.md`](../auth-messages.md).
Command: `opendqi sftr data-quality-pack --mar <auth070.xml> [...]`.

## Business meaning

`auth.070` is the firm-side / TR-relayed **event-driven margin
activity report** for CCP-cleared securities financing
transactions. Each `Rpt` element wraps one margin event (New,
Error, Correction, or Trade Update), carrying the same 6-amount
shape as `auth.085` (the state snapshot) but with per-event
semantics — when did the IM / VM / excess-collateral change,
what was the action type, who was on the other side.

This is the **activity** sister of `auth.085` (state). Together
they triangulate the CCP-cleared margin lifecycle :

- `auth.070` answers *"what happened this margin event"*.
- `auth.085` answers *"what's the latest state of this portfolio"*.

EMIR has no direct equivalent of `auth.070` at the portfolio-margin
level — `auth.108` (EMIR MAR) is per-margin-event but at the
trade level, not the portfolio.

## XSD envelope

```text
Document (urn:iso:std:iso:20022:tech:xsd:auth.070.001.02)
└─ SctiesFincgRptgTxMrgnDataRpt  (V02 root)
   └─ TradData (choice)
      ├─ DataSetActn = "NOTX"   (no-activity report)
      └─ Rpt[]                   (TradeReport21Choice__1)
         └─ <choice: New | Err | Crrctn | TradUpd>
            ├─ TechRcrdId / RptgDtTm / [EvtDt]
            ├─ CtrPty (RptgCtrPty + OthrCtrPty, LEI or natural-person)
            ├─ CollPrtflId  (mandatory)
            ├─ [PstdMrgnOrColl] / InitlMrgn / VartnMrgn / XcssColl
            └─ [RcvdMrgnOrColl] / InitlMrgn / VartnMrgn / XcssColl
```

The `action_type` is encoded in the wrapper element name itself
(unlike `auth.085` which uses a `CtrctMod/ActnTp` leaf): `New →
NEWT`, `Err → ERRT`, `Crrctn → CORR`, `TradUpd → TRDU`. The `Err`
wrapper is metadata-only at the XSD level (no `EvtDt`, no amounts).

## DQI coverage (3 DQIs, v0.18 A4)

| Indicator | Dimension | Rationale |
|---|---|---|
| `DQI_MAR_PARTIAL_SIDES_SFTR` | completeness | share of MAR events reporting only one of (posted, received) |
| `DQI_MAR_EXCESS_COLLATERAL_EVENT_RATE_SFTR` | accuracy | share of posted-side events with `XcssCollPstd > 0` (activity-side mirror of state's `T3_EXCESS_COLLATERAL_USE`) |
| `DQI_MAR_EVENT_SPIKE_SFTR` | timeliness | share of CP-pairs whose event count exceeds mean + 2σ (operational anomaly signal) |

See [`../data-quality-pack.md#indicator-details--sftr`](../data-quality-pack.md#indicator-details--sftr)
for numerator/denominator/threshold detail.

## Granular checks (4 `SFTR.MAR.*`, v0.18 A5)

| Check ID | Dimension | Severity |
|---|---|---|
| `SFTR.MAR.ACTION_TYPE_ENUM_INVALID` | validity | critical |
| `SFTR.MAR.EVENT_WITHOUT_PORTFOLIO` | completeness | high |
| `SFTR.MAR.EVENT_DATE_IN_FUTURE` | validity | high |
| `SFTR.MAR.AMOUNT_CURRENCY_MISSING` | completeness | high |

## Scope notes (v0.18)

- Coverage is intentionally a documented subset of the XSD.
  Untyped leaves go to `raw_fields`. The parser is not a full
  XSD-validating loader — pair with `--xsd <auth.070.001.02.xsd>`
  (via the upstream `xmllint` integration in `opendqi-xml`) for
  structural conformance.
- Negative amounts cannot occur in well-formed XML
  (`ActiveOrHistoricCurrencyAndAmount` is `minInclusive=0` per the
  V02 schema), so there is no `SFTR.MAR.AMOUNT_NEGATIVE` check.
- No cross-batch MAR history tracking (amount-change
  implausibility deferred to v0.19+).
