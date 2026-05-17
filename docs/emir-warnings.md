# EMIR Data-Quality Warnings (auth.106)

`opendqi emir warnings <auth106.xml>` ingests an ISO 20022 `auth.106`
message (`DerivativesTradeWarningsReportV01`, ESMA tag **DATWRN**) —
the Trade Repository's periodic data-quality warnings statistics for
the firm's outstanding / reported derivatives.

This layer is **statistical**: each record summarises, for one
reference date, the report-level missing-valuation /
missing-margin-info / abnormal-values counts and the rates derived
from them. It is distinct from `auth.091` reconciliation statistics
(`EMIR.RST.*`) and from `auth.092` rejection feedback (`EMIR.FBK.*`).
EMIR has no counterparty pairing/reconciliation message. See
[`auth-messages.md`](auth-messages.md) and the per-message note
[`auth-messages/emir-auth106.md`](auth-messages/emir-auth106.md).

## Command

```bash
opendqi emir warnings path/to/auth106.xml --out ./warnings-report
```

Optional flags:

- `--config <yml>` — override the default rate thresholds.
- `--email-config <yml>` — email the report after writing it.

Outputs `summary.json`, `warnings_issues.csv`, `warnings_report.html`.

## Checks (13)

Each fires when the **derived rate exceeds** the configured maximum
(see `WarningsThresholds` in `crates/opendqi-core/src/config.rs`).

### Report-level (5) — one per `RefDt`

| Check ID | Dimension | Severity | Rate (default max) |
|---|---|---|---|
| `EMIR.WRN.MISSING_VALUATION_HIGH` | Completeness | High | `missing_valuation / outstanding_derivatives` (0.05) |
| `EMIR.WRN.OUTDATED_VALUATION_HIGH` | Timeliness | High | `outdated_valuation / outstanding_derivatives` (0.05) |
| `EMIR.WRN.MISSING_MARGIN_INFO_HIGH` | Completeness | High | `missing_margin_info / outstanding_derivatives_margin` (0.05) |
| `EMIR.WRN.OUTDATED_MARGIN_INFO_HIGH` | Timeliness | High | `outdated_margin_info / outstanding_derivatives_margin` (0.05) |
| `EMIR.WRN.ABNORMAL_VALUES_HIGH` | Accuracy | High | `abnormal_values / derivatives_reported` (0.01) |

### Per-counterparty (5) — one per `(RefDt, CtrPty LEI)`

Same rate semantics and thresholds, applied to the `Wrnngs`
breakdown; the offending counterparty LEI is named in the issue.
Folded into the same `warnings_issues.csv`.

| Check ID | Dimension | Severity | Rate (default max) |
|---|---|---|---|
| `EMIR.WRN.CTRPTY_MISSING_VALUATION_HIGH` | Completeness | High | per-CP `missing_valuation / outstanding_derivatives` (0.05) |
| `EMIR.WRN.CTRPTY_OUTDATED_VALUATION_HIGH` | Timeliness | High | per-CP `outdated_valuation / outstanding_derivatives` (0.05) |
| `EMIR.WRN.CTRPTY_MISSING_MARGIN_INFO_HIGH` | Completeness | High | per-CP `missing_margin_info / outstanding_derivatives_margin` (0.05) |
| `EMIR.WRN.CTRPTY_OUTDATED_MARGIN_INFO_HIGH` | Timeliness | High | per-CP `outdated_margin_info / outstanding_derivatives_margin` (0.05) |
| `EMIR.WRN.CTRPTY_ABNORMAL_VALUES_HIGH` | Accuracy | High | per-CP `abnormal_values / derivatives_reported` (0.01) |

### Per-UTI (3) — one per flagged `Wrnngs/TxDtls`

Operational, not statistical: the TR explicitly enumerated these
transactions, so each flagged `TxDtls` yields **one** issue (same
shape as `EMIR.REC.*`), filtered by category. The UTI is set on the
issue; the counterparty LEI is named in the message. Folded into the
same `warnings_issues.csv`. The per-transaction context (valuation /
collateral timestamps, notional, action/event metadata) is preserved
in the record's `raw_fields`, including amount-leaf `Ccy` currency
attributes via the `text|Ccy=XXX` encoding.

| Check ID | Dimension | Severity | Fires |
|---|---|---|---|
| `EMIR.WRN.TX_MISSING_VALUATION` | Completeness | High | once per `TxDtls` the TR flagged for missing valuation |
| `EMIR.WRN.TX_MISSING_MARGIN` | Completeness | High | once per `TxDtls` the TR flagged for missing margin information |
| `EMIR.WRN.TX_ABNORMAL_VALUE` | Accuracy | High | once per `TxDtls` the TR flagged for an abnormal (outlier) value |

A `WrnngsSttstcs/DataSetActn = "NOTX"` no-activity report yields zero
records plus one informational `EMIR.FMT.WRN_NO_RECORDS`.

All three levels (report-level, per-counterparty, per-UTI) are
modelled and kept strictly separate. See
[`auth-messages/emir-auth106.md`](auth-messages/emir-auth106.md)
for the derive maps and limits.
