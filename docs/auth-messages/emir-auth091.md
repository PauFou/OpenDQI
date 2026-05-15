# EMIR `auth.091` — Derivatives Trade Reconciliation Statistical Report

Per-message coverage note. Parent catalog: [`../auth-messages.md`](../auth-messages.md).
Checks reference: [`../emir-recon-stats.md`](../emir-recon-stats.md).

## Business meaning

`auth.091` is the trade repository's periodic **reconciliation
statistics**: per reference date and reconciliation category, how many
transactions are paired/unpaired and reconciled/unreconciled, broken
down by counterparty pair. OpenDQI uses it to flag low pairing /
reconciliation rates and downward trends per counterparty
(`opendqi emir recon-stats`, `EMIR.RST.*`).

## Direction

**TR → firm.**

## Coverage status

**schema-verified (subset, derived).** The parser parses the real
`auth.091.001.02` envelope, but the message has **no explicit rate
fields** — OpenDQI **derives** `pairing_rate`/`recon_rate` by
accumulating cohort transaction counts and projects them onto the
unchanged scalar `ReconStatsRecord`. The derivation is an explicit,
documented interpretation, not a verbatim field mapping.

## Real envelope

```
Document
└─ DerivsTradRcncltnSttstclRpt  (DerivativesTradeReconciliationStatisticalReportV02)
   └─ RcncltnSttstcs  (StatisticsPerCounterparty15Choice — choice)
      ├─ DataSetActn = "NOTX"            (no-activity / empty report)
      └─ Rpt (1..n)  one per (RefDt × reconciliation-category cohort)
         ├─ RefDt  (ISODate)
         ├─ RcncltnCtgrs  (ReportingRequirement2Choice — choice)
         │  ├─ RptgRqrmnt (ReconciliationCategory3):
         │  │     RptgTp, Pairg(PARD|UNPR), Rcncltn(RECO|NREC),
         │  │     ValtnRcncltn(RECO|NREC|NOAP), Rvvd, FrthrMod
         │  └─ NoRptgRqrmnt (ReconciliationCategory2): Rvvd, FrthrMod
         ├─ TtlNbOfTxs  (Number)         (cohort total — context only)
         └─ TxDtls (0..n)  (ReconciliationCounterpartyPairStatistics6)
            ├─ CtrPtyId/RptgCtrPty/LEI
            ├─ TtlNbOfTxs  (Number)      ← per-pair count for THIS cohort
            └─ RcncltnRpt (1..n): TxId/UnqIdr/UnqTxIdr, MtchgCrit
```

Accepted root namespace:
`urn:iso:std:iso:20022:tech:xsd:auth.091.001.02`.

## Derivation → canonical `ReconStatsRecord`

For each counterparty `RptgCtrPty/LEI`, accumulate every
`TxDtls/TtlNbOfTxs` by the enclosing cohort's status:

```
paired       += count   when cohort Pairg   == PARD
unpaired     += count   when cohort Pairg   == UNPR
reconciled   += count   when cohort Rcncltn == RECO
unreconciled += count   when cohort Rcncltn == NREC
```

then emit one record per LEI (sorted, deterministic):

| Canonical field | Source |
|---|---|
| `counterparty_lei` | `TxDtls/CtrPtyId/RptgCtrPty/LEI` |
| `reporting_date` | `Rpt/RefDt` |
| `pairing_rate` | `paired / (paired + unpaired)` (else `None`) |
| `recon_rate` | `reconciled / (reconciled + unreconciled)` (else `None`) |
| `outstanding_paired` / `outstanding_unpaired` | *(none — see Limitations)* |
| `regime` | `Emir` |

`RcncltnSttstcs/DataSetActn = NOTX` → zero records + Info
`EMIR.FMT.RST_NO_RECORDS`.

## Fields ignored / known unsupported branches

`RcncltnRpt` / `MtchgCrit` / `TxId` per-transaction reconciliation
detail, `ValtnRcncltn`, `RptgTp`, `Rvvd`, `FrthrMod`, the cohort-level
`Rpt/TtlNbOfTxs`, `RptSubmitgNtty` / `NttyRspnsblForRpt` / `OthrCtrPty`
LEIs, and **`NoRptgRqrmnt` cohorts** (no Pairg/Rcncltn → contribute
nothing to the derived rates).

### Documented limitations

- **Derived rates.** The real message has no rate field; the rates are
  computed from cohort counts (a defensible interpretation, not a
  verbatim mapping). A faithful hierarchical model is a separate future
  milestone.
- **No outstanding counts.** `outstanding_paired`/`outstanding_unpaired`
  have no real-schema source → left `None`, so
  **`EMIR.RST.OUTSTANDING_UNPAIRED_HIGH` is unreachable** from real
  auth.091 (same honest-limit precedent as
  `EMIR.MSR.HAIRCUT_OUT_OF_RANGE`). `PAIRING_RATE_LOW`,
  `RECON_RATE_LOW` and (with `--prior`) `PAIRING_RATE_TREND_DOWN` are
  reachable via the derived rates.
- **Single reference date assumption.** One `reporting_date` per LEI;
  if a report mixes `RefDt`s the last seen wins (documented).
- **Per-transaction detail dropped.** `RcncltnRpt`/`MtchgCrit` field
  comparisons are not modelled (that is the faithful re-model
  milestone).
- **Not a full XSD validation** — same documented "subset" stance as
  `auth.107`; use `--xsd` with a locally-held official schema for
  strict validation (see [`../xsd-validation.md`](../xsd-validation.md)).

## Schema source used

ESMA EMIR REFIT outgoing-messages usage guideline
**`auth.091.001.02_ESMAUG_DATREC_1.0.0`** (base message
`auth.091.001.02`,
`DerivativesTradeReconciliationStatisticalReportV02`). The
SWIFT-licensed XSD is held **locally only** (`ESMA_docs/`, gitignored)
and is **never** redistributed or excerpted; only element names,
nesting and cardinalities were used to align the parser.

## Verification procedure

1. `cargo test -p opendqi-xml --lib emir_recon_stats`
2. `cargo test -p opendqi-xml --test recon_stats_integration` (derived
   rates, low-rate checks, trend-vs-prior, no-activity path).
3. `opendqi emir recon-stats examples/emir/recon_stats/auth091-sample.xml
   --out /tmp/rst` → `EMIR.RST.PAIRING_RATE_LOW` /
   `EMIR.RST.RECON_RATE_LOW`; add
   `--prior examples/emir/recon_stats/auth091-prior.xml` →
   `EMIR.RST.PAIRING_RATE_TREND_DOWN`;
   `…/auth091-no-records.xml` → zero records + one
   `EMIR.FMT.RST_NO_RECORDS` info note, no error.
4. Optional strict check against a locally-held official XSD (not
   committed): `xmllint --noout --schema <local auth.091 xsd> <file>`
   (fixtures are schema-shaped subsets, so a full pass is not
   asserted).
