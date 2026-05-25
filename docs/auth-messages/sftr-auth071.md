# SFTR `auth.071` — Securities Financing Reporting Transaction Reused Collateral Data Report

Per-message coverage note. Parent catalog: [`../auth-messages.md`](../auth-messages.md).
Command: `opendqi sftr data-quality-pack --reuse-activity <auth071.xml> [...]`.

## Business meaning

`auth.071` is the firm-side **event-driven report of collateral
reused or reinvested**. Each `Rpt` element wraps one reuse event
(New / Error / Correction / CollateralReuseUpdate), declaring
how much of the collateral the firm holds has been re-pledged
or reinvested, in which currency, and at what cash
reinvestment rate.

Reuse / rehypothecation is a SFTR-specific concept (no EMIR
equivalent). The regulator wants visibility into the chain of
re-use to track systemic leverage build-up.

## XSD envelope

```text
Document (urn:iso:std:iso:20022:tech:xsd:auth.071.001.02)
└─ SctiesFincgRptgTxReusdCollDataRpt
   └─ TradData (choice)
      ├─ DataSetActn = "NOTX"
      └─ Rpt[]                   (ReuseDataReport6Choice__1)
         └─ <choice: New | Err | Crrctn | CollReuseUpd>
            ├─ TechRcrdId / RptgDtTm / [EvtDay]
            ├─ CtrPty  (3-entity: RptSubmitgNtty + RptgCtrPty +
            │           NttyRspnsblForRpt, NO OthrCtrPty)
            ├─ [CollCmpnt]
            │  ├─ Scty[]/{ISIN, ReuseVal Actl|Estmtd @Ccy}
            │  └─ [Csh]/{RinvstdCsh[], CshRinvstmtRate}
            └─ [FndgSrc[]] (Tp enum SECL|FREE|OTHR + MktVal)
```

action_type encoded in the wrapper element name: `New → NEWT`,
`Err → ERRT`, `Crrctn → CORR`, `CollReuseUpd → CRUD`. The `Err`
wrapper is metadata-only.

**Key shape divergences vs auth.070** :
- No `OthrCtrPty` (firm-side report, not bilateral).
- No `CollPrtflId` (records keyed by submitter + event-day + ISIN).
- Variable-count `Scty[]` per record — the parser sums every
  `ReuseVal` (Actl + Estmtd uniformly) into a single
  `total_reuse_value` Decimal.

## DQI coverage (2 DQIs, v0.18 B4)

| Indicator | Dimension | Rationale |
|---|---|---|
| `DQI_REUSE_VOLUME_MISSING_SFTR` | completeness | share of non-Err records reporting neither Scty/ReuseVal nor Csh/CshRinvstmtRate |
| `DQI_REUSE_ERR_RETRACTION_RATE_SFTR` | timeliness | share of records that are `ERRT` retractions (high = poor first-shot quality) |

**Honest plan pivot** : the v0.18 plan originally listed
`DQI_REUSE_VOLUME_RATE_SFTR` (TSR UTI cross-ref) and
`DQI_REUSE_CHAIN_DEPTH_SFTR`. XSD verification showed auth.071
carries no UTI cross-reference field and no chain-depth field
— both were redesigned to the indicators above which are
honestly computable from the actual shipped fields.

## Granular checks (2 `SFTR.REU.*`, v0.18 B5)

| Check ID | Dimension | Severity |
|---|---|---|
| `SFTR.REU.MISSING_REUSE_CURRENCY` | completeness | high |
| `SFTR.REU.RATE_OUTSIDE_PLAUSIBLE_BAND` | accuracy | warning |

The plausible band `[-0.05, 0.50]` is a conservative sanity
check on `cash_reinvestment_rate` — values outside it likely
signal a unit error (percentage vs decimal fraction) or a sign
error.

## Scope notes (v0.18)

- Per-ISIN breakdown and `FndgSrc` details land in
  `raw_fields` (the DQI computers and granular checks consume
  the typed aggregate fields only).
- No reuse-chain history tracking — auth.071 reports single-hop
  events; chain depth analysis would need cross-batch state
  (deferred to v0.19+).
