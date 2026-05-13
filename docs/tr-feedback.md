# TR feedback ingestion

OpenDQI reads Trade Repository response messages — the files a TR
sends *back* to a reporting firm — and converts each line into a
`FeedbackRecord`. Combined with the local SQLite history store, this
produces issues that are directly actionable for Errors & Omissions
workflows: confirmed gaps, rejected submissions, inaccurate fields,
TR-vs-firm discrepancies.

Two regimes are supported in v1:

- EMIR: ISO 20022 `auth.092` — Missing / Inaccurate Trade Reports.
- SFTR: ISO 20022 `auth.080` — equivalent for the SFTR regime.

See [`feedback-checks.md`](feedback-checks.md) for the catalog of
`*.FBK.*` checks fired by these messages.

## Usage

```bash
# Prerequisites: at least one prior scan must have populated the store.
opendqi emir scan ./reports/april/ --mapping ./mapping.yml \
    --store ./opendqi-history.db --out ./report-april/

# Ingest the auth.092 file received from the TR.
opendqi emir feedback ./trade-repo/april/auth092-feedback.xml \
    --store ./opendqi-history.db --out ./feedback-april/

# SFTR is parallel.
opendqi sftr feedback ./trade-repo/april/auth080-feedback.xml \
    --store ./opendqi-history.db --out ./feedback-april-sftr/
```

`--store <PATH>` is **required** for `feedback` (it is optional for
`scan`). Without the local history of submissions, the cross-reference
that distinguishes a real gap from a stale feedback file is not
possible, and the feedback ingestion would be reduced to a list of
rejected UTIs with no context.

## Expected XML structure

A feedback document carries a header plus a sequence of `<Sts>` blocks.
Each block contains exactly one of the four status wrappers:

| Wrapper | `FeedbackType` |
|---|---|
| `<Rjctd>` | `Rejected` |
| `<Mssng>` | `Missing` |
| `<Inaccrt>` | `Inaccurate` |
| `<RcncltnBrk>` | `ReconciliationBreak` |

Recognised leaves inside any wrapper:

| Element | Mapped to |
|---|---|
| `UnqTxIdr` | `uti` |
| `RsnCd` | `reason_code` |
| `RsnDesc` | `reason_description` |
| `FldNm` | `reported_field` (used by `Inaccurate`) |

Plus, at the document header:

| Element | Mapped to |
|---|---|
| `Hdr/FdbckDtTm` | `feedback_timestamp` (RFC 3339, applied to every record in the file) |

See `examples/emir/feedback/auth092-sample.xml` and
`examples/sftr/feedback/auth080-sample.xml` for a complete, hand-authored
fixture.

## Legal note

The official SWIFT-licensed XSDs for `auth.092` / `auth.080` are **not
redistributed** with OpenDQI, in the same way the `auth.030` /
`auth.052` XSDs are not. The OpenDQI adapter parses a plausible
structure inspired by the public ISO 20022 catalog. If the schema you
receive from your TR differs in element names (e.g. `<MissingReport>`
instead of `<Mssng>`), edit the leaf table in
`crates/opendqi-xml/src/feedback.rs` accordingly — the parser
architecture is robust to that change.

## Storage and lifecycle

For v1 the feedback batch is **not** persisted into the SQLite store.
The store remains an audit trail of *submissions*. A future milestone
will add a `feedbacks` table and a status column so that the user can
mark each TR signal as Open / Resolved / Stale and let the next scan
pick up where the previous one left off.

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
