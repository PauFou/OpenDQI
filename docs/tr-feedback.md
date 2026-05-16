# TR feedback ingestion

OpenDQI reads Trade Repository response messages — the files a TR
sends *back* to a reporting firm — and converts each line into a
`FeedbackRecord`. Combined with the local SQLite history store, this
produces issues that are directly actionable for Errors & Omissions
workflows: confirmed gaps, rejected submissions, inaccurate fields,
TR-vs-firm discrepancies.

Feedback ingestion is **EMIR-only**: ISO 20022 `auth.092`
(Derivatives Trade Rejection Statistical Report). SFTR has no
rejection-feedback message — real `auth.080` is a *reconciliation
status advice*, handled by `opendqi sftr reconcile`
(see [`auth-messages/sftr-auth080.md`](auth-messages/sftr-auth080.md)).

See [`feedback-checks.md`](feedback-checks.md) for the catalog of
`EMIR.FBK.*` checks, and
[`auth-messages/emir-auth092.md`](auth-messages/emir-auth092.md) for
the real `auth.092` envelope and the extracted-field map.

## Usage

```bash
# Prerequisites: at least one prior scan must have populated the store.
opendqi emir scan ./reports/april/ --mapping ./mapping.yml \
    --store ./opendqi-history.db --out ./report-april/

# Ingest the auth.092 file received from the TR.
opendqi emir feedback ./trade-repo/april/auth092-feedback.xml \
    --store ./opendqi-history.db --out ./feedback-april/
```

`--store <PATH>` is **required** for `feedback` (it is optional for
`scan`). Without the local history of submissions, the cross-reference
that distinguishes a real gap from a stale feedback file is not
possible, and the feedback ingestion would be reduced to a list of
rejected UTIs with no context.

## Expected XML structure

The EMIR path parses the **real** ESMA usage-guideline envelope
`auth.092.001.04` (`DerivativesTradeRejectionStatisticalReportV04`) —
a rejection *statistics* report. The extracted-field map (UTI from
`TxId/UnqIdr/UnqTxIdr`, the repeating `DtldVldtnRule/Id` list, `Sts`,
`RptgTmStmp`), the ignored branches and the documented limits are in
[`auth-messages/emir-auth092.md`](auth-messages/emir-auth092.md). See
`examples/emir/feedback/auth092-sample.xml` for a complete,
hand-authored, schema-shaped fixture.

## Legal note

The official SWIFT-licensed `auth.092` XSD is **not redistributed**
with OpenDQI, in the same way the `auth.030` / `auth.052` XSDs are
not. Only the schema *shape* is encoded; the per-message coverage note
documents the subset OpenDQI consumes.

## Storage and lifecycle

The feedback batch is persisted into the `feedbacks` table of the
SQLite history store. Each row starts `open` and can be transitioned
to `resolved` / `stale` via the top-level `opendqi feedback
list/resolve/stale` workflow — see
[`history-store.md`](history-store.md#feedbacks-table--workflow).

## Diagnostics

The CLI prints a one-line summary plus the path to the generated HTML
report:

```
Ingested 5 feedback record(s). 4 issues (2 critical, 2 high). Score: 86.0/100.
Report: ./feedback-april/report.html
```

The same three outputs as `scan` are produced: `summary.json`,
`issues.csv`, `report.html`. The score is computed on the feedback
batch size (not the underlying submission counts), since the firm's
target is to drive these to zero.
