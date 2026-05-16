# SFTR `auth.080` — Securities Financing Reporting Reconciliation Status Advice

Per-message coverage note. Parent catalog: [`../auth-messages.md`](../auth-messages.md).
Command: `opendqi sftr reconcile` (re-homed from `sftr feedback`).

## Business meaning

`auth.080` is the trade repository's **reconciliation status advice**
for SFTs: per transaction, whether the two counterparties' submissions
are paired and reconciled, and — when not matched — which fields
disagree. It is the SFTR analogue of EMIR `auth.106`; it is **not**
rejection feedback. OpenDQI uses it to flag unreconciled SFTs and
field-level mismatches (`SFTR.REC.*`).

## Direction

**TR → firm.**

## Coverage status

**schema-verified (subset).** The real `auth.080.001.02` envelope is
parsed (`crates/opendqi-xml/src/reconciliation.rs`,
`read_sftr_reconciliation_xml` dispatches by namespace) and projected
onto the existing scalar `ReconciliationRecord` — a deliberate,
documented derive-subset. No model/check/store-schema change.

## Real envelope

```
Document
└─ SctiesFincgRptgRcncltnStsAdvc  (SecuritiesFinancingReportingReconciliationStatusAdviceV02)
   └─ RcncltnData  (TradeData34Choice — choice)
      ├─ DataSetActn = "NOTX"            (no-activity / empty report)
      └─ Rpt (1..n)  (TradeData28)
         ├─ PairgRcncltnSts (1..5): DtldNbOfRpts + DtldSts
         │     (CLRC|LNRC|PARD|RECO|UNPR)        — summary counts only
         └─ RcncltnRpt (1..n)  (ReconciliationReport8)  ← RECORD
            ├─ TechRcrdId
            ├─ TxId/UnqTradIdr , TxId/RptgCtrPty/LEI ,
            │  TxId/OthrCtrPty/Lgl/LEI , TxId/NttyRspnsblForRpt/LEI
            ├─ Modfd (bool)
            └─ RcncltnSts (choice):
               ├─ NoRcncltnReqrd
               └─ RptgData (choice):
                  ├─ Mtchd
                  └─ NotMtchd → CtrPty1, CtrPty2,
                       MtchgCrit/{CtrPtyMtchgCrit | LnMtchgCrit |
                       CollMtchgCrit}/<criterion> (Compare* Val1/Val2)
```

Accepted root namespace:
`urn:iso:std:iso:20022:tech:xsd:auth.080.001.02` (mismatch →
`SFTR.FMT.XML_UNSUPPORTED_NAMESPACE`, with a hint that `sftr reconcile`
also accepts the synthetic `auth.083.001.01`).

## Derive map → `ReconciliationRecord` (model unchanged)

| Canonical field | Source |
|---|---|
| `uti` | `RcncltnRpt/TxId/UnqTradIdr` |
| `reporting_counterparty` | `RcncltnRpt/TxId/RptgCtrPty/LEI` |
| `other_counterparty` | `RcncltnRpt/TxId/OthrCtrPty/Lgl/LEI` |
| `pairing_status` / `reconciliation_status` | `RcncltnSts`: `Mtchd` → `PAIRED`/`RECONCILED`; `NotMtchd` → `PAIRED`/`UNRECONCILED`; `NoRcncltnReqrd` → both `None` |
| `mismatched_fields` | the element names of the Compare* leaves under `NotMtchd/MtchgCrit/{CtrPty,Ln,Coll}MtchgCrit` (order-preserving, deduped) |
| `regime` | `Sftr` |

`RcncltnData/DataSetActn = "NOTX"` → zero records + Info
`SFTR.FMT.RCNCLN_NO_RECORDS`.

Reachable checks: `SFTR.REC.UNRECONCILED_TRADE`,
`SFTR.REC.FIELD_MISMATCH` (via `NotMtchd`).

## Fields ignored / known unsupported branches

`PairgRcncltnSts` summary counts; the Compare* `Val1`/`Val2` value
pairs (only the criterion *name* is kept, not the differing values);
`Modfd`; `TechRcrdId`; `NttyRspnsblForRpt`; the natural-person
(`Ntrl`) other-counterparty branch; `CtrPty1`/`CtrPty2` under
`NotMtchd`.

### Documented limitations

- **`SFTR.REC.UNPAIRED_TRADE` is unreachable from real auth.080.**
  "Unpaired" (`UNPR`) is only a `PairgRcncltnSts` *summary* count, not
  a per-`RcncltnRpt` state — same honest-limit precedent as
  `EMIR.MSR.HAIRCUT_OUT_OF_RANGE` / `EMIR.RST.OUTSTANDING_UNPAIRED_HIGH`.
- **`mismatched_fields` are criterion names, not value pairs.** The
  scalar `ReconciliationRecord` cannot hold the `Val1`/`Val2` pairs; a
  faithful value-pair model is a separate future milestone.
- **No per-record reconciliation timestamp** in auth.080 →
  `reconciliation_timestamp` stays `None`.
- **SFTR has no rejection-feedback message.** auth.080 being
  reconciliation means the `SFTR.FBK.*` checks and the synthetic
  `sftr feedback` path have no real SFTR input (the latter is retained
  only as an inert placeholder).
- **Not a full XSD validation** — same documented "subset" stance as
  `auth.107`; use `--xsd` with a locally-held official schema for
  strict validation (see [`../xsd-validation.md`](../xsd-validation.md)).

## Schema source used

ESMA SFTR TR-to-Authority usage guideline
**`auth.080.001.02_ESMAUG_SFTREC_1.1.0`** (base message
`auth.080.001.02`,
`SecuritiesFinancingReportingReconciliationStatusAdviceV02`). The
SWIFT-licensed XSD is held **locally only** (`ESMA_docs/`, gitignored)
and is **never** redistributed or excerpted; only element names,
nesting and cardinalities were used.

## Verification procedure

1. `cargo test -p opendqi-xml --lib reconciliation`
2. `cargo test -p opendqi-xml --test sftr_auth080_integration`
3. `opendqi sftr reconcile examples/sftr/reconciliation/auth080-sample.xml
   --store /tmp/r080.db --out /tmp/r080` → `SFTR.REC.UNRECONCILED_TRADE`
   + `SFTR.REC.FIELD_MISMATCH`, no `SFTR.REC.UNPAIRED_TRADE`;
   `…/auth080-no-records.xml` → zero records + one
   `SFTR.FMT.RCNCLN_NO_RECORDS` info note; re-open the same store
   (migration idempotent). `opendqi sftr feedback <real auth.080>`
   cleanly warns unsupported-namespace (no mis-parse).
4. Optional strict check against a locally-held official XSD (not
   committed): `xmllint --noout --schema <local auth.080 xsd> <file>`
   (fixtures are schema-shaped subsets).
