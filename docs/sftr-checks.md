# SFTR data-quality checks

OpenDQI currently ships **5 MVP SFTR checks**, all triggered by
`opendqi sftr scan`. Each check operates on `SftrRecord` slices via
the [`opendqi_core::SftrCheck`] trait.

| Check ID | Dimension | Severity | What it detects |
|---|---|---|---|
| `SFTR.COMP.UTI_MISSING` | Completeness | High | UTI absent or empty |
| `SFTR.COMP.COLLATERAL_VALUE_MISSING` | Completeness | High | Outstanding SFT with no collateral value |
| `SFTR.COMP.HAIRCUT_MISSING` | Completeness | Warning | Collateral value present but no haircut |
| `SFTR.TIM.LATE_REPORTING` | Timeliness | High | Reporting delay (reporting - event) above the configured threshold |
| `SFTR.UNI.DUPLICATE_UTI` | Uniqueness | Critical | Same UTI reported on multiple active records |

The `LATE_REPORTING` threshold is shared with EMIR
(`timeliness.max_reporting_delay_hours`, default 24h). All other
checks are parameter-free.

## Roadmap

Future checks (require additional historical or schema context):

- Reuse indicator coherence vs. collateral re-use limit
- Haircut bounds (negative / above 1.0)
- Settlement date before execution date
- Repo rate / lending fee plausibility
- Collateral ISIN format (12 chars ISO 6166)
- LEI format checks on counterparties (already covered for EMIR; will
  port to SFTR via a shared format helper)

## Adding a check

Each check lives under `crates/opendqi-core/src/dq/sftr/`. Implement
the `SftrCheck` trait, add a positive + negative unit test, then
register the type in `default_sftr_checks()` in
`crates/opendqi-core/src/dq/mod.rs`.
