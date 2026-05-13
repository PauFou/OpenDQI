# TR reconciliation ingestion

OpenDQI reads the Trade Repository's pairing / reconciliation
reports — the files a TR sends back to a firm summarising whether
its submitted trades have been paired with the counterparty's, and
which fields disagree.

Two regimes are supported:

- EMIR: ISO 20022 `auth.106`.
- SFTR: ISO 20022 `auth.083`.

See [`reconciliation-checks.md`](reconciliation-checks.md) for the
catalog of `*.REC.*` checks fired by these messages.

## Usage

```bash
# Prerequisite: at least one prior scan has populated the store.
opendqi emir scan ./reports/april/ --mapping ./mapping.yml \
    --store ./opendqi-history.db --out ./report-april/

# Ingest the auth.106 reconciliation file from the TR.
opendqi emir reconcile ./trade-repo/april/auth106-reconciliation.xml \
    --store ./opendqi-history.db --out ./reconciliation-april/

# SFTR is parallel.
opendqi sftr reconcile ./trade-repo/april/auth083-reconciliation.xml \
    --store ./opendqi-history.db --out ./reconciliation-april-sftr/
```

`--store` is required (records are persisted into the
`reconciliations` table).

## Expected XML structure

A reconciliation document carries a header plus a sequence of
`<Rcncltn>` blocks. Recognised leaves inside each block:

| Element | Mapped to |
|---|---|
| `UnqTxIdr` | `uti` |
| `RptgCtrPty/LEI` | `reporting_counterparty` |
| `OthrCtrPty/LEI` | `other_counterparty` |
| `PrngSts` | `pairing_status` (`PAIRED` / `UNPAIRED`) |
| `RcncltnSts` | `reconciliation_status` (`RECONCILED` / `UNRECONCILED`) |
| `MismatchedField` (repeating) | each element appended to `mismatched_fields` |

Plus, at the document header:

| Element | Mapped to |
|---|---|
| `Hdr/RcncltnDtTm` | `reconciliation_timestamp` (RFC 3339, applied to every record) |

See `examples/emir/reconciliation/auth106-sample.xml` and
`examples/sftr/reconciliation/auth083-sample.xml` for complete
hand-authored fixtures.

## Legal note

The official SWIFT-licensed XSDs for `auth.106` / `auth.083` are
**not redistributed** with OpenDQI. The adapter parses a plausible
structure aligned with public ISO 20022 catalog conventions. If the
real schema you receive uses different element names, edit the leaf
table in `crates/opendqi-xml/src/reconciliation.rs` — the parser
architecture is robust to renames.
