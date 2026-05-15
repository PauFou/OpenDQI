# EMIR `auth.092` — Derivatives Trade Rejection Statistical Report (DATREJ)

Per-message coverage note. Parent catalog: [`../auth-messages.md`](../auth-messages.md).
Workflow: `opendqi emir feedback` + the top-level
`opendqi feedback list/resolve/stale/analytics`.

## Business meaning

`auth.092` is **not** a per-UTI "rejected / missing / inaccurate"
feed. It is the *Derivatives Trade Rejection Statistical Report*: the
trade repository's periodic statistics of how many reports/transactions
it accepted vs rejected, per counterparty, with — per rejected
transaction — the list of ESMA validation rules that failed and a
status code. OpenDQI consumes the per-transaction rejection rows to
feed its rejection workflow (open/resolved/stale, analytics,
`rejection_profile.yml`, the `EMIR.PSC.*` pre-submission loop).

## Direction

**TR → firm.**

## Coverage status

**schema-verified (subset).** The EMIR side of
`crates/opendqi-xml/src/feedback.rs` parses the real
`auth.092.001.04` envelope, but **projects** each rejected transaction
onto OpenDQI's unchanged scalar `FeedbackRecord`. It is a deliberate,
documented lossy projection — not a full faithful model of the
statistical report.

## Real envelope

```
Document
└─ DerivsTradRjctnSttstclRpt   (DerivativesTradeRejectionStatisticalReportV04)
   └─ RjctnSttstcs   (StatisticsPerCounterparty18Choice — choice)
      ├─ DataSetActn = "NOTX"             (no-activity / empty report)
      └─ Rpt
         ├─ RefDt , TtlNbOfRpts , TtlNbOfRpts{Accptd,Rjctd} ,
         │  TtlNbOfTxs , TtlNbOfTxs{Accptd,Rjctd}   (aggregates — ignored)
         └─ RjctnSttstcs (1..n)  (RejectionStatistics9)
            ├─ CtrPtyId , RptSttstcs                 (aggregates — ignored)
            └─ DerivSttstcs   (DetailedTransactionStatistics7Choice — choice)
               ├─ DataSetActn = "NOTX"
               └─ DtldSttstcs
                  ├─ TtlNbOfTxs{,Accptd,Rjctd}       (aggregates — ignored)
                  └─ TxsRjctnsRsn (0..500000)  (RejectionReason71) ← RECORD
                     ├─ TxId/UnqIdr/UnqTxIdr (or TxId/UnqIdr/Prtry/Id)
                     ├─ TxId/RptgTmStmp
                     ├─ Sts  (ReportingMessageStatus2Code:
                     │        ACPT | RJCT | INCF | CRPT | NAUT)
                     └─ DtldVldtnRule (0..n)
                        (GenericValidationRuleIdentification1: Id [+ Desc])
```

Accepted root namespace:
`urn:iso:std:iso:20022:tech:xsd:auth.092.001.04` (mismatch →
`EMIR.FMT.XML_UNSUPPORTED_NAMESPACE`; non-well-formed →
`EMIR.FMT.XML_NOT_WELLFORMED`).

## Fields extracted (canonical `FeedbackRecord`)

| Canonical field | Real auth.092 path (relative to `TxsRjctnsRsn`) |
|---|---|
| `uti` | `TxId/UnqIdr/UnqTxIdr` (or `TxId/UnqIdr/Prtry/Id`) |
| `feedback_timestamp` | `TxId/RptgTmStmp` |
| `feedback_type` | derived from `Sts`: `RJCT\|INCF\|CRPT\|NAUT` → `Rejected`; `ACPT` → record skipped |
| `reason_code` | **first** `DtldVldtnRule/Id` |
| `reason_description` | first `DtldVldtnRule/Desc` |
| `regime` | `Emir` |

`reported_field` is always `None` (no inaccurate-field concept in
auth.092). `EMIR.FMT.FBK_NO_RECORDS` (Info) is emitted when
`RjctnSttstcs/DataSetActn = NOTX` yields zero rejected transactions.

## Fields ignored / known unsupported branches

All per-report and per-counterparty aggregate counts
(`TtlNbOf*`), `CtrPtyId`, `RptSttstcs`, `RefDt`, the `TxId`
sub-fields beyond UTI + reporting timestamp (action type, derivative
event, other counterparty, collateral portfolio), and **every
validation rule after the first** are not carried by the scalar model.

### Documented limitations

- **Lossy projection.** Real auth.092 lists *several* `DtldVldtnRule`
  per rejected transaction; the scalar `reason_code`/`reason_description`
  keep only the **first**. Faithful capture needs a model +
  SQLite-schema change (separate future milestone).
- **No Missing / Inaccurate.** auth.092 has no such branch. Of the four
  `EMIR.FBK.*` checks, only **`EMIR.FBK.TR_REJECTED_UTI`** is reachable
  from real auth.092; `TR_MISSING_DESPITE_SUBMISSION`,
  `TR_MISSING_BUT_NOT_SENT` and `TR_INACCURATE_REPORTED` are
  unreachable from this message (same honest-limit precedent as
  `EMIR.MSR.HAIRCUT_OUT_OF_RANGE`).
- **Accepted rows skipped.** The statistical report also enumerates
  `Sts=ACPT` transactions; these are not feedback and produce no
  `FeedbackRecord`.
- **Aggregates dropped.** Per-counterparty accept/reject totals are not
  modelled (the model is per-transaction).
- **Not a full XSD validation.** Element paths are aligned and
  test-exercised; OpenDQI does not assert a fully valid
  `auth.092.001.04` instance. Use `--xsd` with a locally-held official
  schema for strict validation (see [`../xsd-validation.md`](../xsd-validation.md)).

## Schema source used

ESMA EMIR REFIT outgoing-messages usage guideline
**`auth.092.001.04_ESMAUG_DATREJ_1.0.0`** (base message
`auth.092.001.04`, `DerivativesTradeRejectionStatisticalReportV04`).
The SWIFT-licensed XSD is held **locally only** (`ESMA_docs/`,
gitignored) and is **never** redistributed or excerpted; only element
names, nesting and cardinalities were used to align the parser.

## Verification procedure

1. `cargo test -p opendqi-xml --lib feedback`
2. `cargo test -p opendqi-xml --test feedback_integration` (parse the
   schema-shaped fixture, run the feedback pack, assert
   `EMIR.FBK.TR_REJECTED_UTI` fires; the no-activity path).
3. `opendqi emir feedback examples/emir/feedback/auth092-sample.xml
   --store /tmp/fbk-smoke.db --out /tmp/fbk` → an
   `EMIR.FBK.TR_REJECTED_UTI` issue; and
   `…/auth092-no-records.xml` → zero records + one
   `EMIR.FMT.FBK_NO_RECORDS` info note, no error.
4. Optional strict check against a locally-held official XSD (not
   committed): `xmllint --noout --schema <local auth.092 xsd> <file>`
   (fixtures are schema-shaped subsets, so a full pass is not
   asserted).
