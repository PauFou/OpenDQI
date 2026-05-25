# SFTR `auth.086` — Securities Financing Reporting Reused Collateral Data Transaction State Report

Per-message coverage note. Parent catalog: [`../auth-messages.md`](../auth-messages.md).
Command: `opendqi sftr data-quality-pack --reuse-state <auth086.xml> [...]`.

## Business meaning

`auth.086` is the TR's **portfolio-level state snapshot of
reused collateral**. Each `Stat` element reports the latest
reuse / reinvestment state for one CCP-cleared SFT portfolio :
how much of the collateral pool is currently re-pledged, what
cash reinvestment rate applies, etc.

This is the **state** sister of `auth.071` (event log) :

- `auth.071` answers *"what reuse event happened"*.
- `auth.086` answers *"what's the latest reuse state of the portfolio"*.

Structurally `auth.086` combines two patterns we've seen
elsewhere — `auth.085`-style envelope (`Stat[]` blocks with a
single shape + `CtrctMod/ActnTp` leaf for `action_type`) +
`auth.071`-style content (`CollCmpnt/Scty + Csh` + `FndgSrc`).

## XSD envelope

```text
Document (urn:iso:std:iso:20022:tech:xsd:auth.086.001.02)
└─ SctiesFincgRptgReusdCollDataTxStatRpt
   └─ TradData (choice)
      ├─ DataSetActn = "NOTX"
      └─ Stat[]                  (ReuseDataReportCorrection15__1)
         ├─ TechRcrdId
         ├─ CtrPty (3-entity: RptSubmitgNtty + RptgCtrPty +
         │          NttyRspnsblForRpt, NO OthrCtrPty)
         ├─ [CollCmpnt]
         │  ├─ Scty[]/{ISIN, ReuseVal Actl|Estmtd @Ccy}
         │  └─ [Csh]/{RinvstdCsh[], CshRinvstmtRate}
         ├─ EvtDay (ISODate)
         ├─ RptgDtTm (ISONormalisedDateTime)
         ├─ [FndgSrc[]]
         └─ CtrctMod/ActnTp     → typically "REUU"
```

action_type comes from the `CtrctMod/ActnTp` leaf instead of a
wrapper element name (mirror of `auth.085`'s pattern). The
canonical value is `REUU` (CollateralReuseUpdate) per the
`TransactionOperationType6Code` enum.

## DQI coverage (2 DQIs, v0.18 C4)

| Indicator | Dimension | Rationale |
|---|---|---|
| `DQI_REUSE_STATE_VOLUME_MISSING_SFTR` | completeness | share of state snapshots reporting no Scty/ReuseVal and no Csh/CshRinvstmtRate |
| `DQI_REUSE_STATE_STALE_SFTR` | timeliness | share of snapshots whose `RptgDtTm` is older than the configured TARGET2 business-day threshold |

**Honest divergence vs the auth.071 sister** : the `auth.086`
version of `VOLUME_MISSING` does NOT exclude `Err`-style
retraction records — auth.086 has no action-wrapper concept
(it's state, not events), so every `Stat` counts toward the
denominator. The auth.071 sister excludes `ERRT` records.

Stale threshold reuses
`thresholds.timeliness.max_valuation_age_business_days` (same
field as `DQI_T3_MARGIN_STALE_SFTR` on auth.085) — same
staleness intuition transfers across SFTR state messages.

## Granular checks (2 `SFTR.REU.STATE.*`, v0.18 C5)

| Check ID | Dimension | Severity |
|---|---|---|
| `SFTR.REU.STATE.MISSING_REUSE_CURRENCY` | completeness | high |
| `SFTR.REU.STATE.RATE_OUTSIDE_PLAUSIBLE_BAND` | accuracy | warning |

State-side mirror of the `SFTR.REU.*` family on `auth.071`.
Same band `[-0.05, 0.50]`, same rationale (unit / sign error
catch).

## Scope notes

- Per-ISIN breakdown + `FndgSrc` details captured into
  `raw_fields`.
- The `Stat[]` element type is `ReuseDataReportCorrection15__1`
  — the same name appears in auth.071's `Crrctn` wrapper, but
  the auth.086 usage is state-snapshot (no wrapper choice
  surrounding it).
