# EMIR data-quality checks

OpenDQI currently ships **21 EMIR data-quality checks**, 16 of which
are aligned with the official ESMA EMIR Refit Validation Rules
(`EMIR-VR-*`). Each check is a pure function from a slice of
`EmirRecord` to a list of `DqIssue`. The full registry is exposed
through [`opendqi_core::dq::default_checks`].

Severity scale: `info` < `warning` < `high` < `critical`.

## Catalog

| Check ID | Dimension | Severity | ESMA ref | What it detects |
|---|---|---|---|---|
| `EMIR.COMP.UTI_MISSING` | Completeness | High | — | UTI absent or empty |
| `EMIR.COMP.VALUATION_MISSING` | Completeness | High | — | Outstanding trade with no valuation amount |
| `EMIR.COMP.COUNTERPARTY_1_MISSING` | Completeness | High | EMIR-VR-1003 | Reporting counterparty LEI absent |
| `EMIR.COMP.COUNTERPARTY_2_MISSING` | Completeness | High | EMIR-VR-1006 | Other counterparty LEI absent |
| `EMIR.COMP.NOTIONAL_CURRENCY_MISSING` | Completeness | High | EMIR-VR-2003 | Notional amount present, currency missing |
| `EMIR.COMP.VALUATION_CURRENCY_MISSING` | Completeness | High | — | Valuation amount present, currency missing |
| `EMIR.COMP.VALUATION_TIMESTAMP_MISSING` | Completeness | High | — | Valuation amount present, timestamp missing |
| `EMIR.VLD.LEI_FORMAT_RC` | Validity | High | EMIR-VR-1003-01 | Reporting counterparty LEI not ISO 17442 shape (20 chars, 18 alphanumeric + 2 digits) |
| `EMIR.VLD.LEI_FORMAT_OC` | Validity | High | EMIR-VR-1006-01 | Other counterparty LEI not ISO 17442 shape |
| `EMIR.VLD.LEI_FORMAT_ERR` | Validity | High | EMIR-VR-1004-01 | Entity-responsible-for-reporting LEI not ISO 17442 shape |
| `EMIR.VLD.CURRENCY_NOTIONAL` | Validity | Warning | ISO 4217 | Notional currency not three uppercase letters |
| `EMIR.VLD.CURRENCY_VALUATION` | Validity | Warning | ISO 4217 | Valuation currency not three uppercase letters |
| `EMIR.ACC.ABNORMAL_MATURITY` | Accuracy | Warning | — | Maturity is a placeholder, too far in the future, or precedes effective date |
| `EMIR.ACC.ZERO_NOTIONAL` | Accuracy | Warning | — | Notional is exactly zero on a non-position record |
| `EMIR.ACC.NEGATIVE_NOTIONAL` | Accuracy | High | — | Notional is negative |
| `EMIR.UNI.DUPLICATE_UTI` | Uniqueness | Critical | — | Same UTI reported on multiple active records |
| `EMIR.TIM.LATE_REPORTING` | Timeliness | High | — | Reporting delay (reporting - event) above the configurable threshold |
| `EMIR.TIM.VALUATION_AFTER_REPORTING` | Timeliness | Warning | EMIR-VR-1001-05 | Valuation timestamp is after the reporting timestamp |
| `EMIR.CON.REPORTING_BEFORE_EXECUTION` | Consistency | High | EMIR-VR-1001-04 | Reporting timestamp precedes the execution timestamp |
| `EMIR.CON.CLEARED_REQUIRES_CCP` | Consistency | High | — | Clearing status indicates "cleared" but no CCP LEI provided |
| `EMIR.CON.VALUATION_AFTER_TERMINATION` | Consistency | High | — | Valuation observed after the termination date |

`EMIR-VR-*` references map to the official ESMA EMIR Refit
Validation Rules workbook published by ESMA (not redistributed by
OpenDQI; see [`iso20022-emir.md`](iso20022-emir.md) for where to
obtain it).

## Configuration

A few checks take inputs from the YAML thresholds file
(`opendqi emir scan --config thresholds.yml`):

- `EMIR.TIM.LATE_REPORTING` → `timeliness.max_reporting_delay_hours`
  (default `24`)
- `EMIR.ACC.ABNORMAL_MATURITY` → `maturity.abnormal_maturity_years`
  (default `51`) and `maturity.placeholder_dates`
  (default `["1900-01-01", "2099-12-31", "9999-12-31"]`)

All other checks are parameter-free.

## Adding a new check

Each check lives in its own file under
`crates/opendqi-core/src/dq/`. The pattern is mechanical: declare a
unit struct, implement the `Check` trait, write at least one
positive and one negative unit test, then add the type to
`default_checks()` in `crates/opendqi-core/src/dq/mod.rs`.

Format helpers (LEI / ISO 4217 shape) live in
`crates/opendqi-core/src/dq/formats.rs`.

## Severity rationale

- **Critical**: irrecoverable defects that block downstream
  processing (malformed XML, duplicate UTI, XSD violation never
  reaches the TR).
- **High**: defects that would typically be rejected by the TR or
  fail post-submission reconciliation (missing UTI, invalid LEI
  shape, cleared without CCP).
- **Warning**: observations that suggest a problem but might be
  legitimate or operational (zero notional, valuation slightly after
  reporting, placeholder maturity).
- **Info**: notes that do not require remediation
  (unknown-but-tolerated XML element).
