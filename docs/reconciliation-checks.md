# Reconciliation (TR ↔ counterparty) checks

> **Naming caveat.** The official ESMA `auth.106` (EMIR) and
> `auth.083` (SFTR) messages appear in the ISO 20022 catalog as
> data-quality warning messages, **not** as pairing / matching
> reports. The OpenDQI parser today reads a synthetic pairing /
> matching structure with `<Rcncltn>` blocks; the 6 `*.REC.*` checks
> documented below operate on that shape. This is a useful
> placeholder if a firm receives a matching-style file from its TR,
> but it diverges semantically from the official message. The
> resolution is on the roadmap (Phase 3 — Rejection analytics): we
> will either extend this parser or add a dedicated `tr-warnings`
> parser alongside it. See [`auth-messages.md`](auth-messages.md)
> for the canonical message catalog.

OpenDQI's "reconciliation" checks ingest TR pairing / matching
reports following the synthetic structure described above. Each
report lists, per UTI, whether the TR has been able to pair the
firm's submission with the counterparty's, and whether the two
submissions agree on the reported fields.

Reconciliation checks run via the `reconcile` subcommand on each
regime:

```bash
opendqi emir reconcile <auth.106.xml> --store <history.db> --out <dir>
opendqi sftr reconcile <auth.083.xml> --store <history.db> --out <dir>
```

`--store` is **required**: the records are persisted into the
`reconciliations` table for audit, and the prior history allows
richer messages (e.g. mentioning the prior reporting counterparty).

See [`tr-reconciliation.md`](tr-reconciliation.md) for the expected
XML structure and legal note.

## Catalog

### EMIR (3)

| Check ID | Dimension | Severity | What it detects |
|---|---|---|---|
| `EMIR.REC.UNPAIRED_TRADE` | Consistency | High | TR reports the trade as UNPAIRED — the counterparty has not submitted a matching report. |
| `EMIR.REC.UNRECONCILED_TRADE` | Consistency | High | TR reports the trade as UNRECONCILED — paired with the counterparty but one or more fields disagree. |
| `EMIR.REC.FIELD_MISMATCH` | Accuracy | High | One issue per field name in the TR's `<MismatchedField>` list. |

### SFTR (3)

Parallel checks for the SFTR regime:

| Check ID | Dimension | Severity | What it detects |
|---|---|---|---|
| `SFTR.REC.UNPAIRED_TRADE` | Consistency | High | TR reports the SFT as UNPAIRED. |
| `SFTR.REC.UNRECONCILED_TRADE` | Consistency | High | TR reports the SFT as UNRECONCILED. |
| `SFTR.REC.FIELD_MISMATCH` | Accuracy | High | One issue per mismatched field name. |

## Storage

Every record parsed from an `auth.106` / `auth.083` file is persisted
into the `reconciliations` table in the SQLite history store, with
columns mirroring the canonical `ReconciliationRecord` plus an
`ingested_at` timestamp. Mismatched-field lists are serialised as
JSON arrays. See [`history-store.md`](history-store.md#schema).

The `reconciliations` table is currently **append-only** — there is
no `status` workflow for v1 (the `feedbacks` workflow handles the
related but distinct E&O loop). A future milestone may add a
`status` column and a top-level `opendqi reconcile list/resolve` if
the use case emerges.

## Adding a reconciliation check

EMIR checks live under `crates/opendqi-core/src/dq/reconciliation/`,
SFTR checks under `crates/opendqi-core/src/dq/sftr/reconciliation/`.
Implement `ReconciliationCheck` / `SftrReconciliationCheck`, add
positive and negative unit tests, then register the struct in
`default_reconciliation_checks()` / `default_sftr_reconciliation_checks()`.
