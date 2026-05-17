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

- `--email-config <yml>` — email the report after writing it.

Outputs `summary.json`, `missing_collateral_issues.csv`,
`missing_collateral_report.html`. There is no `--store` / `--prior`
(stateless request scan).

## Checks (2)

| Check ID | Dimension | Severity | Fires |
|---|---|---|---|
| `SFTR.MCR.MISSING_COLLATERAL_REQUESTED` | Completeness | High | once per `TxId` — the TR is requesting the missing collateral for this SFT |
| `SFTR.MCR.MISSING_UTI_ON_REQUEST` | Validity | High | when the `TxId` has no `UnqTradIdr` — the request cannot be tied to a booked SFT |

`TxId` is mandatory (`minOccurs="1"`) in the real schema, so a valid
instance always produces at least one record — there is **no**
no-activity / `*_NO_RECORDS` info path (unlike auth.080 / auth.091 /
auth.106).

For the real envelope, the derive map onto `MissingCollateralRecord`
(including the natural-person `OthrCtrPty/Ntrl/Id/Id` branch) and the
documented limitations (no store cross-reference,
`OthrMstrAgrmtDtls` dropped, not a full XSD validation), see
[`auth-messages/sftr-auth083.md`](auth-messages/sftr-auth083.md).
