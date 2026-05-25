# SFTR `auth.084` — Securities Financing Reporting Transaction Status Advice

Per-message coverage note. Parent catalog: [`../auth-messages.md`](../auth-messages.md).
Command: `opendqi sftr data-quality-pack --tr-status-advice <auth084.xml> [...]`.

## Business meaning

`auth.084` is the trade repository's **aggregate rejection
statistics report** for the firm's SFTR submissions over a
reporting period. One file → one logical record carrying
totals (number of reports sent, accepted, rejected) plus a
per-error-code breakdown of which validation rules tripped the
rejections.

This is the SFTR analogue of **EMIR `auth.092`** (TR-side
trade-report feedback). Two structural differences :

- `auth.092` is per-trade (one feedback row per submitted UTI).
- `auth.084` is aggregate (one statistics row per file).

Both feed the same family of DQ questions — *"how clean is our
upstream reporting pipeline?"* — but the SFTR side gives you a
single rate and a per-rule breakdown rather than a per-trade
diagnostic.

## XSD envelope

```text
Document (urn:iso:std:iso:20022:tech:xsd:auth.084.001.02)
└─ SctiesFincgRptgTxStsAdvc
   └─ TxRptStsAndRsn (TradeData35Choice__1)
      └─ Rpt (TradeData29__1, exactly 1)
         ├─ RptSttstcs (DetailedReportStatistics5)
         │  ├─ TtlNbOfRpts                    → total_reports
         │  ├─ TtlNbOfRptsAccptd              → total_reports_accepted
         │  ├─ TtlNbOfRptsRjctd               → total_reports_rejected
         │  └─ NbOfRptsRjctdPerErr[]
         │     ├─ Dtl/.../Id                  → rejected_reports_per_error key
         │     └─ NbOfTxs                     → rejected_reports_per_error value
         └─ TxSttstcs                         → raw_fields
```

## DQI coverage (1 DQI, v0.18 D2)

| Indicator | Dimension | Rationale |
|---|---|---|
| `DQI_REJ_RATE_SFTR` | accuracy | `sum(total_reports_rejected) / sum(total_reports)` across the input slice; mirror of EMIR `DQI_REJ_RATE` |

Threshold default: 5 % amber / 20 % red — calibrated against
EMIR `DQI_REJ_RATE`. The evidence table surfaces the top-N
validation rule codes by per-error count.

## Granular checks

None in v0.18. The aggregate-statistics shape doesn't have
per-record fields to check beyond what `DQI_REJ_RATE_SFTR`
already aggregates. A future
`SFTR.TSA.SUM_PER_ERROR_NEQ_TOTAL_REJECTED` consistency check
could detect arithmetic mismatches (rejected total ≠ sum of
per-error counts) but is deferred to v0.19+ when real-world
fixtures surface a need.

## Plan pivot honesty

The v0.18 plan originally scoped Phase D for `auth.078`
(Pairing Request). XSD verification against the ESMA bundle
showed `auth.078` is NOT in the published message set —
pairing semantics are carried by `auth.080` (already covered
since v0.17). Pivoted Phase D to `auth.084` (real shipped SFTR
message, real coverage gap closed).

## Scope notes

- 1 record per file in practice (the XSD allows only one `Rpt`
  child under `TxRptStsAndRsn`).
- Multiple files aggregate naturally: the DQI computer sums
  totals + merges per-error breakdowns across the input slice.
