# EMIR data-quality checks

OpenDQI currently ships **51 EMIR data-quality checks** covering all
six DQ dimensions. Most are aligned with the official ESMA EMIR
Refit Validation Rules (`EMIR-VR-*`). Each check is a pure function
from a slice of `EmirRecord` to a list of `DqIssue`. The full
registry is exposed through [`opendqi_core::dq::default_checks`].

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
| `EMIR.COMP.CLEARING_STATUS_MISSING` | Completeness | High | — | `clearing_status` absent |
| `EMIR.COMP.INTRAGROUP_INDICATOR_MISSING` | Completeness | Warning | — | Intragroup transaction flag absent |
| `EMIR.COMP.NATURE_MISSING` | Completeness | Warning | — | Nature of the reporting counterparty absent |
| `EMIR.COMP.TRADING_CAPACITY_MISSING` | Completeness | Warning | — | Trading capacity absent |
| `EMIR.COMP.MASTER_AGREEMENT_TYPE_MISSING` | Completeness | Warning | — | Master agreement type absent |
| `EMIR.COMP.ASSET_CLASS_MISSING` | Completeness | High | — | Asset class absent |
| `EMIR.COMP.INITIAL_MARGIN_MISSING_FOR_FULL` | Completeness | Warning | — | `collateralisation_category=FLCL` but `initial_margin_posted` absent |
| `EMIR.COMP.VARIATION_MARGIN_MISSING_FOR_FULL` | Completeness | Warning | — | `collateralisation_category=FLCL` but `variation_margin_posted` absent |
| `EMIR.COMP.COLLATERAL_PORTFOLIO_REQUIRED_FOR_FULL` | Completeness | High | — | `collateralisation_category=FLCL` but no collateral portfolio code |
| `EMIR.VLD.LEI_FORMAT_CCP` | Validity | High | — | Clearing CCP LEI not ISO 17442 shape |
| `EMIR.VLD.ACTION_TYPE_ENUM` | Validity | High | — | Action type not in `{NEWT, MODI, CORR, ETRM, POSC, VALU, MARU, OTHR, EROR, REVI}` |
| `EMIR.VLD.EVENT_TYPE_ENUM` | Validity | Warning | — | Event type not in the ESMA enumeration |
| `EMIR.VLD.VALUATION_TYPE_ENUM` | Validity | Warning | — | Valuation type not in `{MTMA, MTMO}` |
| `EMIR.VLD.TRADING_CAPACITY_ENUM` | Validity | Warning | — | Trading capacity not in `{AGEN, PRIN}` |
| `EMIR.VLD.ASSET_CLASS_ENUM` | Validity | High | — | Asset class outside the EMIR enumeration |
| `EMIR.VLD.NATURE_ENUM` | Validity | Warning | — | Nature not in `{F, N, C}` |
| `EMIR.VLD.MASTER_AGREEMENT_TYPE_ENUM` | Validity | Warning | — | Master agreement type outside the recognised set |
| `EMIR.VLD.COLLATERALISATION_CATEGORY_ENUM` | Validity | Warning | — | Collateralisation category not in `{FLCL, OWCL, PRCL, UNCL}` |
| `EMIR.VLD.CLEARING_STATUS_ENUM` | Validity | High | — | Clearing status not in `{CLRD, NCLR, ICLR, INCL}` |
| `EMIR.ACC.NEGATIVE_INITIAL_MARGIN_POSTED` | Accuracy | High | — | Initial margin posted is negative |
| `EMIR.ACC.NEGATIVE_INITIAL_MARGIN_COLLECTED` | Accuracy | High | — | Initial margin collected is negative |
| `EMIR.ACC.NEGATIVE_VARIATION_MARGIN_POSTED` | Accuracy | High | — | Variation margin posted is negative |
| `EMIR.ACC.NEGATIVE_VARIATION_MARGIN_COLLECTED` | Accuracy | High | — | Variation margin collected is negative |
| `EMIR.CON.NCLR_FORBIDS_CCP` | Consistency | Warning | — | Clearing status is NCLR but a CCP LEI is reported |
| `EMIR.CON.MARU_REQUIRES_MARGIN` | Consistency | High | — | Action type is MARU but no margin amount is set |
| `EMIR.CON.EVENT_BEFORE_EXECUTION` | Consistency | High | — | Event timestamp precedes execution timestamp |
| `EMIR.CON.MATURITY_IN_PAST` | Consistency | Warning | — | Outstanding trade with `maturity_date` already in the past |
| `EMIR.CON.TERMINATION_AFTER_MATURITY` | Consistency | High | — | Termination date is after maturity date |
| `EMIR.CON.EFFECTIVE_AFTER_MATURITY` | Consistency | High | — | Effective date is after maturity date |
| `EMIR.CON.ETRM_REQUIRES_TERMINATION_DATE` | Consistency | High | — | Action ETRM with no `termination_date` |

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
