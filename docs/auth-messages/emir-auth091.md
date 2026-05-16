# EMIR `auth.091` — Derivatives Trade Reconciliation Statistical Report

Per-message coverage note. Parent catalog: [`../auth-messages.md`](../auth-messages.md).
Checks reference: [`../emir-recon-stats.md`](../emir-recon-stats.md).

## Business meaning

`auth.091` is the trade repository's periodic **reconciliation
statistics**: per reference date and reconciliation category, how many
transactions are paired/unpaired and reconciled/unreconciled, broken
down by counterparty pair, **and** a per-transaction
`RcncltnRpt`/`MtchgCrit` detail. OpenDQI uses it to flag low pairing /
reconciliation rates and downward trends per counterparty
(`EMIR.RST.*`) and, from the per-transaction detail, unpaired /
unreconciled trades and field-level mismatches (`EMIR.REC.*`) — both
via `opendqi emir recon-stats`.

## Direction

**TR → firm.**

## Coverage status

**schema-verified (subset, derived).** The parser parses the real
`auth.091.001.02` envelope, but the message has **no explicit rate
fields** — OpenDQI **derives** `pairing_rate`/`recon_rate` by
accumulating cohort transaction counts and projects them onto the
unchanged scalar `ReconStatsRecord`. The per-transaction
`TxDtls/RcncltnRpt` detail is additionally projected onto the
unchanged scalar `ReconciliationRecord` (status inherited from the
enclosing cohort; mismatch inferred from `MtchgCrit` `Val1`≠`Val2`).
Both are explicit, documented interpretations, not verbatim field
mappings; no canonical-model / check / store-schema change.

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
            ├─ CtrPtyId/RptgCtrPty/LEI , CtrPtyId/OthrCtrPty/…/LEI
            ├─ TtlNbOfTxs  (Number)      ← per-pair count for THIS cohort
            └─ RcncltnRpt (1..n)  (ReconciliationReport14)
               ├─ TxId/UnqIdr/UnqTxIdr   (or TxId/UnqIdr/Prtry/Id)
               └─ MtchgCrit  (MatchingCriteria16):
                    {CtrPty,Ctrct,Valtn,Tx}MtchgCrit
                      /<criterion>  (Compare* : Val1 / Val2)
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

## Derivation → canonical `ReconciliationRecord` (per transaction)

Each `TxDtls/RcncltnRpt` also yields one `ReconciliationRecord`. The
message carries **no per-record matched/notmatched marker** (unlike
SFTR `auth.080`): a criterion counts as mismatched when its `Val1`
text ≠ `Val2` text, and pairing/recon status is **inherited from the
enclosing cohort**.

| Canonical field | Source |
|---|---|
| `uti` | `RcncltnRpt/TxId/UnqIdr/UnqTxIdr` (else `…/Prtry/Id`) |
| `reporting_counterparty` | enclosing `TxDtls/CtrPtyId/RptgCtrPty/LEI` |
| `other_counterparty` | `TxDtls/CtrPtyId/OthrCtrPty/…/LEI` |
| `pairing_status` | cohort `Pairg`: `PARD`→`PAIRED`, `UNPR`→`UNPAIRED`; `NoRptgRqrmnt`→`None` |
| `reconciliation_status` | cohort `Rcncltn`: `RECO`→`RECONCILED`, `NREC`→`UNRECONCILED`; else `None` |
| `mismatched_fields` | `MtchgCrit/{CtrPty,Ctrct,Valtn,Tx}MtchgCrit/<criterion>` element names where the concatenated `Val1` text ≠ `Val2` text (order-preserving, deduped) |
| `reconciliation_timestamp` | *(none — see Limitations)* |
| `regime` | `Emir` |

These feed the existing `EMIR.REC.*` checks
([`../reconciliation-checks.md`](../reconciliation-checks.md)) and are
folded into `recon_stats_issues.csv` by `opendqi emir recon-stats`
(also in the local web UI's recon-stats operation — shared core, no
duplicated logic). All three are reachable from real auth.091:
`EMIR.REC.UNPAIRED_TRADE` (UNPR cohorts), `EMIR.REC.UNRECONCILED_TRADE`
(NREC cohorts), `EMIR.REC.FIELD_MISMATCH` (`Val1`≠`Val2`).
`recon-stats` persists nothing — no `--store`, no store-schema change.

## Fields ignored / known unsupported branches

The `MtchgCrit` `Val1`/`Val2` **value pairs** (only the criterion
*name* is kept, not the differing values), `ValtnRcncltn`, `RptgTp`,
`Rvvd`, `FrthrMod`, the cohort-level `Rpt/TtlNbOfTxs`,
`RptSubmitgNtty` / `NttyRspnsblForRpt`, the natural-person other-
counterparty branch, and **`NoRptgRqrmnt` cohorts** (no Pairg/Rcncltn →
contribute nothing to the derived rates and no inherited per-tx
status).

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
- **Per-tx status is cohort-inherited, not per-record.** auth.091's
  `RcncltnRpt` has no own pairing/recon marker; the status is taken
  from the enclosing `Rpt` cohort's `Pairg`/`Rcncltn`. A
  `NoRptgRqrmnt` cohort yields per-tx records with no status (no
  `EMIR.REC.*` from them).
- **Mismatch is inferred, names only.** A criterion is "mismatched"
  when its concatenated `Val1` text ≠ `Val2` text;
  `mismatched_fields` holds the **criterion element names**, not the
  differing value pairs (the scalar `ReconciliationRecord` cannot hold
  them — same precedent as SFTR `auth.080`).
- **No per-record reconciliation timestamp** in auth.091 →
  `reconciliation_timestamp` stays `None`.
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
   `EMIR.RST.RECON_RATE_LOW` **and**, from the per-transaction detail,
   `EMIR.REC.UNPAIRED_TRADE` / `EMIR.REC.UNRECONCILED_TRADE` /
   `EMIR.REC.FIELD_MISMATCH` in `recon_stats_issues.csv`; add
   `--prior examples/emir/recon_stats/auth091-prior.xml` →
   `EMIR.RST.PAIRING_RATE_TREND_DOWN`;
   `…/auth091-no-records.xml` → zero records + one
   `EMIR.FMT.RST_NO_RECORDS` info note, no error.
4. Optional strict check against a locally-held official XSD (not
   committed): `xmllint --noout --schema <local auth.091 xsd> <file>`
   (fixtures are schema-shaped subsets, so a full pass is not
   asserted).
