# EMIR Margin Activity (MAR) & Margin State (MSR) checks

OpenDQI's margin layers ingest the two ISO 20022 messages dedicated
to OTC-derivatives margin in EMIR:

- **MAR** (`auth.108`) — the **activity** history: every individual
  margin call, posting, collection, correction.
- **MSR** (`auth.109`) — the **state** snapshot: the TR's current
  view of margin postings per outstanding portfolio.

Both are post-TR feedback messages (TR → firm). OpenDQI does not
build them, it ingests them.

## CLI

```bash
opendqi emir mar-scan <auth.108.xml> [--store <db>] --out <dir>
opendqi emir msr-scan <auth.109.xml> [--store <db>] --out <dir>
```

Outputs are deliberately distinct from other layers so they can
co-exist in the same `--out` directory:

- MAR: `summary.json`, `mar_issues.csv`, `mar_report.html`.
- MSR: `summary.json`, `msr_issues.csv`, `msr_report.html`.

## MAR catalog (8 checks)

| Check ID | Dimension | Severity | What it detects |
|---|---|---|---|
| `EMIR.MAR.MARGIN_TYPE_ENUM` | Validity | High | `action_type` not in {MARU, MARV, MARC, MARN}. |
| `EMIR.MAR.POSTED_NEGATIVE` | Accuracy | High | Initial or variation margin posted < 0. |
| `EMIR.MAR.COLLECTED_NEGATIVE` | Accuracy | High | Initial or variation margin collected < 0. |
| `EMIR.MAR.LARGE_MARGIN_DELTA` | Accuracy | Warning | Same portfolio: consecutive IM/VM posted jump > 50%. |
| `EMIR.MAR.MARGIN_NEEDS_CURRENCY` | Consistency | High | Margin amount present but `margin_currency` is missing. |
| `EMIR.MAR.PORTFOLIO_CODE_MISSING` | Completeness | High | `collateral_portfolio_code` absent on a margin record. |
| `EMIR.MAR.TIMELINESS` | Timeliness | High | `reporting_timestamp - event_timestamp` exceeds the configured threshold (default 24h). |
| `EMIR.MAR.DUPLICATE_MARGIN_CALL` | Uniqueness | High | Same `(collateral_portfolio_code, event_timestamp)` reported more than once in a batch. |

## MSR catalog (8 checks)

| Check ID | Dimension | Severity | What it detects |
|---|---|---|---|
| `EMIR.MSR.INITIAL_MARGIN_NEGATIVE` | Accuracy | Critical | Current IM posted or collected < 0. |
| `EMIR.MSR.VARIATION_MARGIN_NEGATIVE` | Accuracy | Critical | Current VM posted or collected < 0. |
| `EMIR.MSR.COLLATERAL_MARKET_VALUE_NEGATIVE` | Accuracy | Critical | `collateral_market_value` < 0. |
| `EMIR.MSR.MARGIN_STALE` | Timeliness | High | Header `state_as_of` older than `timeliness.max_valuation_age_business_days` relative to `ctx.now`. |
| `EMIR.MSR.MARGIN_MISSING_FOR_OUTSTANDING` | Completeness | High | UTI listed in the MSR but every margin amount is missing. |
| `EMIR.MSR.HAIRCUT_OUT_OF_RANGE` | Accuracy | Warning | `haircut_applied` < 0 or > 1. |
| `EMIR.MSR.COLLATERALIZATION_CATEGORY_ENUM` | Validity | High | `collateralization_category` not in {FCOL, PCOL, UCOL, OCOL}. |
| `EMIR.MSR.IM_POSTED_VS_COLLECTED_IMBALANCE` | Accuracy | Warning | IM posted vs IM collected differ by > 10% on the same row. |

## Design notes

- Each record type has its own struct
  (`MarginActivityRecord`, `MarginStateRecord`) — margin events and
  margin state have very different field sets, and forcing a single
  type would require many `Option<_>`s.
- Each layer has its own trait (`MarginActivityCheck`,
  `MarginStateCheck`) — keeps the runner signatures honest and
  prevents accidental cross-layer mixing.
- MAR and MSR checks live under
  `crates/opendqi-core/src/dq/margin_activity/` and
  `crates/opendqi-core/src/dq/margin_state/`. Each file holds one
  check + two unit tests (positive + negative).
- The XML adapters are the canonical reference for the supported
  leaf table (`crates/opendqi-xml/src/emir_mar.rs`,
  `emir_msr.rs`). Their element paths are aligned with the real ESMA
  EMIR REFIT usage guidelines `auth.108.001.01_ESMAUG_DATMDA_1.1.0`
  and `auth.109.001.01_ESMAUG_DATMDS_1.1.0` — extracted-subset map,
  ignored branches and documented limitations in
  [`auth-messages/emir-auth108.md`](auth-messages/emir-auth108.md)
  and [`auth-messages/emir-auth109.md`](auth-messages/emir-auth109.md).
