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

## Checks (5)

Each fires when the **derived rate exceeds** the configured maximum
(see `WarningsThresholds` in `crates/opendqi-core/src/config.rs`).

| Check ID | Dimension | Severity | Rate (default max) |
|---|---|---|---|
| `EMIR.WRN.MISSING_VALUATION_HIGH` | Completeness | High | `missing_valuation / outstanding_derivatives` (0.05) |
| `EMIR.WRN.OUTDATED_VALUATION_HIGH` | Timeliness | High | `outdated_valuation / outstanding_derivatives` (0.05) |
| `EMIR.WRN.MISSING_MARGIN_INFO_HIGH` | Completeness | High | `missing_margin_info / outstanding_derivatives_margin` (0.05) |
| `EMIR.WRN.OUTDATED_MARGIN_INFO_HIGH` | Timeliness | High | `outdated_margin_info / outstanding_derivatives_margin` (0.05) |
| `EMIR.WRN.ABNORMAL_VALUES_HIGH` | Accuracy | High | `abnormal_values / derivatives_reported` (0.01) |

A `WrnngsSttstcs/DataSetActn = "NOTX"` no-activity report yields zero
records plus one informational `EMIR.FMT.WRN_NO_RECORDS`.

The per-counterparty `Wrnngs` breakdown (with LEIs and per-UTI
`TxDtls`) is a documented deferred subset — these checks operate on
the report-level aggregate. See
[`auth-messages/emir-auth106.md`](auth-messages/emir-auth106.md) for
the derive map and limits.
