# Feedback (TR → firm) checks

OpenDQI's "feedback" checks ingest the EMIR Trade Repository response
message (ISO 20022 `auth.092`) and cross-reference it against the local
SQLite history store. They surface the **Errors & Omissions** signals
the TR sends back to the firm in a form that is directly actionable.

**SFTR has no rejection-feedback message.** Real `auth.080` is a
*reconciliation status advice* (handled by `opendqi sftr reconcile` →
`SFTR.REC.*`, see [`auth-messages/sftr-auth080.md`](auth-messages/sftr-auth080.md)).
The synthetic SFTR feedback command and the `SFTR.FBK.*` checks were
removed in Milestone 0.4; feedback is EMIR-only.

Feedback checks run via the EMIR `feedback` subcommand:

```bash
opendqi emir feedback <auth.092.xml> --store <history.db> --out <dir>
```

`--store` is **required** here: without the history of prior
submissions, the checks cannot distinguish a real gap from a stale
feedback file.

See [`tr-feedback.md`](tr-feedback.md) for the expected XML structure
and legal note on the SWIFT-licensed schemas.

## Catalog

### EMIR (4)

| Check ID | Dimension | Severity | What it detects |
|---|---|---|---|
| `EMIR.FBK.TR_REJECTED_UTI` | Validity | Critical | The Trade Repository rejected a report (validation failure). Message includes the reason code and description. |
| `EMIR.FBK.TR_MISSING_BUT_NOT_SENT` | Completeness | High | The TR signals a UTI as missing and the local history store has **no prior NEWT** for it — a confirmed submission gap. |
| `EMIR.FBK.TR_MISSING_DESPITE_SUBMISSION` | Consistency | Critical | The TR signals a UTI as missing but the local history store **does** record a prior NEWT for it — either a TR ingestion failure or a stale feedback file. |
| `EMIR.FBK.TR_INACCURATE_REPORTED` | Accuracy | High | The TR accepted the report but flagged one or more inaccurate fields. Message includes the flagged field and the TR's reason. |

### SFTR

None — SFTR has no rejection-feedback message (see the note above).
The SFTR equivalent of the TR→firm matching signal is the
reconciliation status advice (`auth.080` → `SFTR.REC.*`, via
`opendqi sftr reconcile`).

### Reserved

`<RcncltnBrk>` blocks in the feedback message are ingested into the
`FeedbackRecord` model as `FeedbackType::ReconciliationBreak`, but no
check fires for them in v1. Acting on a reconciliation break requires
matching two feedback files (firm A's and counterparty B's) and is
scoped to a future milestone.

## Semantics

Each feedback check sees two inputs:

- **feedback**: records parsed from the `auth.092` file.
- **prior**: every prior EMIR record in the store whose UTI appears in
  the feedback (no `scan_id` filter — for feedback, "what is known" is
  the right semantic).

Each ingested feedback batch is **persisted into the `feedbacks`
table** in the history store. Use `opendqi feedback list/resolve/stale`
to manage the Open/Resolved/Stale workflow — see
[`history-store.md`](history-store.md#feedbacks-table--workflow) for
the catalogue of CLI verbs.

## Adding a feedback check

EMIR feedback checks live under `crates/opendqi-core/src/dq/feedback/`.
Implement `FeedbackCheck`, add positive and negative unit tests, then
register the struct in `default_feedback_checks()` inside
`crates/opendqi-core/src/dq/mod.rs`. (There is no SFTR feedback
registry — SFTR has no rejection-feedback message.)
