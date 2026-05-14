# SFTR margin lending — `SFTR.MAR.*` + `SFTR.MSR.*`

## Why this is not a separate ISO 20022 message

Unlike EMIR (which has dedicated `auth.108` / `auth.109` for margin
activity / margin state on OTC derivatives), the SFTR regime has
**no separate margin reporting message**. Margin lending under SFTR
flows inline:

- in `auth.052` (SFT Trade Report) via `<LnData><MgnLndg>...</MgnLndg>`
  for the SFT type, or via `<MrgnUpd>` action wrapper for margin
  updates (action_type `MARU`);
- in `auth.079` (SFT Trade State Report) via TSR rows whose
  `<SftTp>` is `MGLD`.

Consequently, OpenDQI's SFTR margin lending layer is **a logical
layer** built on top of the existing `SftrRecord` and
`SftrTrStateRecord` types: there is no new sub-command, no new record
type, no new XML adapter. The checks filter `sft_type == "MGLD"` (or
`action_type == "MARU"`) internally and run as part of the
single-batch SFTR layer (`opendqi sftr scan`) and the TSR layer
(`opendqi sftr tr-state-scan`).

This mirrors the spirit of the EMIR `MAR/MSR` layers while staying
honest about ESMA SFTR's structure.

## Activity layer — 5 `SFTR.MAR.*` checks

Operate on `SftrRecord`. Registered in `default_sftr_checks()`. Fire
from `opendqi sftr scan` and from any other SFTR runner that exercises
the single-batch catalog (`tr-audit`, etc.).

| Check ID | Dim / Sev | Trigger |
|---|---|---|
| `SFTR.MAR.MGLD_NEEDS_LOAN_VALUE` | Completeness / High | MGLD outstanding without `loan_value`. |
| `SFTR.MAR.MGLD_NEEDS_COLLATERAL` | Completeness / High | MGLD with `loan_value` but no `collateral_value`. |
| `SFTR.MAR.MARU_REQUIRES_VALUE_OR_HAIRCUT` | Consistency / High | `action_type=MARU` but none of `loan_value`, `collateral_value`, `haircut` is set. |
| `SFTR.MAR.MARU_REQUIRES_PORTFOLIO` | Consistency / High | `action_type=MARU` but `collateral_portfolio_code` absent. |
| `SFTR.MAR.MGLD_HAIRCUT_OUT_OF_RANGE` | Accuracy / Warning | MGLD with `haircut < 0` or `haircut > 1`. (Overlaps with `SFTR.ACC.HAIRCUT_OUT_OF_RANGE` by design — the MAR-tagged ID lets a reader filter the margin layer cleanly.) |

## State layer — 6 `SFTR.MSR.*` checks

Operate on `SftrTrStateRecord`. Registered in
`default_sftr_tr_state_checks()`. Fire from
`opendqi sftr tr-state-scan` and `sftr tr-audit`.

| Check ID | Dim / Sev | Trigger |
|---|---|---|
| `SFTR.MSR.MGLD_OUTSTANDING_NEEDS_LOAN` | Completeness / High | MGLD row outstanding in the TSR without `loan_value`. |
| `SFTR.MSR.MGLD_HAIRCUT_OUT_OF_RANGE` | Accuracy / Warning | MGLD row with `haircut` outside `[0, 1]`. |
| `SFTR.MSR.MGLD_COLLATERAL_UNDER_LOAN` | Accuracy / Warning | `collateral_value < loan_value × (1 - haircut)` — under-collateralisation. Skipped when haircut is out of range (already flagged by the previous check). |
| `SFTR.MSR.MGLD_REUSE_REQUIRES_PORTFOLIO` | Completeness / Warning | MGLD row with `reuse_indicator=true` but no `collateral_portfolio_code`. |
| `SFTR.MSR.MGLD_LOAN_COLL_CURRENCY_MISMATCH` | Validity / Warning | MGLD row where `loan_currency ≠ collateral_currency`. |
| `SFTR.MSR.MGLD_MISSING_ISIN` | Completeness / Warning | MGLD outstanding with `collateral_value` reported but no `collateral_isin` — collateral traceability is expected for margin lending. |

## Design notes

- **Single-file modules**: all activity checks live in
  `crates/opendqi-core/src/dq/sftr/margin_activity.rs`; all state
  checks in `crates/opendqi-core/src/dq/sftr/margin_state.rs`. Each
  file holds a private `is_mgld()` (and `is_outstanding()`) helper, the
  six check structs + `impl SftrCheck` (resp. `impl SftrTrStateCheck`),
  and one paired flag/accept test per check.
- **No new trait**: the checks reuse `SftrCheck` (single-batch) and
  `SftrTrStateCheck` (TSR snapshot), filtering MGLD/MARU at the top of
  `run`. Same pattern as Article 11 `is_uncleared` in
  `crates/opendqi-core/src/dq/risk_mitigation.rs`.
- **No cross-batch check in this layer**: drift detection on the MGLD
  collateral value lands in the lifecycle layer
  (`SFTR.TST.LFC.COLLATERAL_VALUE_REGRESSION`, separately scoped).

## Fixtures

- `examples/sftr/tr_activity/auth052-tar-sample.xml` — 4 MGLD/MARU
  rows added (UTI suffix `-MGLD-` / `-MARU-`), each exercising one
  `SFTR.MAR.*` check.
- `examples/sftr/tr_state/auth079-sample.xml` — 2 MGLD rows added,
  exercising five of the six `SFTR.MSR.*` checks (the
  currency-mismatch case is unit-tested).

Run:

```bash
opendqi sftr scan examples/sftr/tr_activity/auth052-tar-sample.xml \
  --out ./report-mar/
grep 'SFTR.MAR' ./report-mar/issues.csv | cut -d, -f1 | sort -u

opendqi sftr tr-state-scan examples/sftr/tr_state/auth079-sample.xml \
  --out ./report-msr/
grep 'SFTR.MSR' ./report-msr/tr_state_issues.csv | cut -d, -f1 | sort -u
```
