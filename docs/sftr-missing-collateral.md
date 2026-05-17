# SFTR Missing Collateral Request (auth.083)

`opendqi sftr missing-collateral <auth083.xml>` ingests an ISO 20022
`auth.083` message
(`SecuritiesFinancingReportingMissingCollateralRequestV02`) — the
Trade Repository's **operational request asking the firm to supply the
collateral information missing for a list of SFTs**.

This layer is **operational, not statistical**: every transaction in
the message is an actionable item, not an aggregate rate. It is
distinct from the real `auth.080` reconciliation status advice
(`SFTR.REC.*`) and is **not** rejection feedback (SFTR has no feedback
message). See [`auth-messages.md`](auth-messages.md) and the
per-message note
[`auth-messages/sftr-auth083.md`](auth-messages/sftr-auth083.md).

## Command

```bash
opendqi sftr missing-collateral path/to/auth083.xml --out ./mcr-report
```

Optional flags:

- `--tsr <auth079.xml>` — companion SFTR Trade State Report; the
  requested UTIs are cross-referenced against it.
- `--store <db>` — when `--tsr` is not given, use the latest persisted
  SFTR trade state for the requested UTIs (read-only).
- `--email-config <yml>` — email the report after writing it.

`--tsr` takes precedence over `--store` when both are given. Outputs
`summary.json`, `missing_collateral_issues.csv`,
`missing_collateral_report.html`.

## Checks (5)

### Base (2) — always run

| Check ID | Dimension | Severity | Fires |
|---|---|---|---|
| `SFTR.MCR.MISSING_COLLATERAL_REQUESTED` | Completeness | High | once per `TxId` — the TR is requesting the missing collateral for this SFT |
| `SFTR.MCR.MISSING_UTI_ON_REQUEST` | Validity | High | when the `TxId` has no `UnqTradIdr` — the request cannot be tied to a booked SFT |

### Cross-reference (3) — only with `--tsr` / `--store`

Per requested UTI, matched against the firm's SFTR trade state.
No-UTI records are skipped; with neither flag these no-op (output
byte-identical).

| Check ID | Dimension | Severity | Fires |
|---|---|---|---|
| `SFTR.MCR.COLLATERAL_PRESENT_IN_TSR` | Consistency | Info | the TR state already shows collateral (value > 0 or an ISIN) — likely satisfied / TR lag |
| `SFTR.MCR.STILL_MISSING_IN_TSR` | Consistency | High | the SFT is in the TR state but still has no collateral — gap confirmed |
| `SFTR.MCR.REQUESTED_UTI_NOT_IN_TSR` | Consistency | High | the requested SFT is absent from the firm's TR state |

`TxId` is mandatory (`minOccurs="1"`) in the real schema, so a valid
instance always produces at least one record — there is **no**
no-activity / `*_NO_RECORDS` info path (unlike auth.080 / auth.091 /
auth.106).

For the real envelope, the derive map onto `MissingCollateralRecord`
(including the natural-person `OthrCtrPty/Ntrl/Id/Id` branch),
`OthrMstrAgrmtDtls` → `raw_fields`, the cross-ref precedence/limits
and the XSD-subset stance, see
[`auth-messages/sftr-auth083.md`](auth-messages/sftr-auth083.md).
