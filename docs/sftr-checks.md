# SFTR data-quality checks

OpenDQI currently ships **20 SFTR data-quality checks**, exposed
through [`opendqi_core::default_sftr_checks`] and runnable via
`opendqi sftr scan`. The catalog mirrors the EMIR check coverage —
same dimensions, similar logic, SFTR-specific field semantics
(loan / collateral / haircut / settlement).

Severity scale: `info` < `warning` < `high` < `critical`.

## Catalog

| Check ID | Dimension | Severity | What it detects |
|---|---|---|---|
| `SFTR.COMP.UTI_MISSING` | Completeness | High | UTI absent or empty |
| `SFTR.COMP.COLLATERAL_VALUE_MISSING` | Completeness | High | Outstanding SFT with no collateral value |
| `SFTR.COMP.HAIRCUT_MISSING` | Completeness | Warning | Collateral value present but no haircut |
| `SFTR.COMP.COUNTERPARTY_1_MISSING` | Completeness | High | Reporting counterparty LEI absent |
| `SFTR.COMP.COUNTERPARTY_2_MISSING` | Completeness | High | Other counterparty LEI absent |
| `SFTR.COMP.LOAN_CURRENCY_MISSING` | Completeness | High | Loan value present, currency missing |
| `SFTR.COMP.COLLATERAL_CURRENCY_MISSING` | Completeness | High | Collateral value present, currency missing |
| `SFTR.VLD.LEI_FORMAT_RC` | Validity | High | Reporting counterparty LEI not ISO 17442 shape |
| `SFTR.VLD.LEI_FORMAT_OC` | Validity | High | Other counterparty LEI not ISO 17442 shape |
| `SFTR.VLD.LEI_FORMAT_ERR` | Validity | High | Entity-responsible-for-reporting LEI not ISO 17442 shape |
| `SFTR.VLD.CURRENCY_LOAN` | Validity | Warning | Loan currency not three uppercase letters |
| `SFTR.VLD.CURRENCY_COLLATERAL` | Validity | Warning | Collateral currency not three uppercase letters |
| `SFTR.VLD.ISIN_COLLATERAL` | Validity | Warning | Collateral ISIN not ISO 6166 shape (2 letters + 9 alphanumeric + 1 digit) |
| `SFTR.ACC.NEGATIVE_LOAN` | Accuracy | High | Loan value is negative |
| `SFTR.ACC.NEGATIVE_COLLATERAL` | Accuracy | High | Collateral value is negative |
| `SFTR.ACC.HAIRCUT_OUT_OF_RANGE` | Accuracy | Warning | Haircut < 0 or > 1.0 |
| `SFTR.UNI.DUPLICATE_UTI` | Uniqueness | Critical | Same UTI on multiple active SFT records |
| `SFTR.TIM.LATE_REPORTING` | Timeliness | High | Reporting delay above the configured threshold |
| `SFTR.CON.SETTLEMENT_BEFORE_EXECUTION` | Consistency | High | Settlement date precedes execution timestamp |
| `SFTR.CON.MATURITY_BEFORE_EFFECTIVE` | Consistency | High | Maturity date precedes effective date |

## XSD validation

`opendqi sftr scan ... --xsd <path>` and `opendqi sftr validate
<input> --xsd <path>` use the existing [`xmllint`-based
validator](xsd-validation.md). Get the official SWIFT-licensed XSD
from ESMA / MyStandards and point `--xsd` at the local file.
Violations are reported as `SFTR.FMT.XSD_VIOLATION` (high) and
`SFTR.FMT.XSD_TOOL_ERROR` (warning) in the regular `issues.csv`,
with the verbatim xmllint errors written to a dedicated
`xsd_errors.csv`.

## Configuration

`SFTR.TIM.LATE_REPORTING` consumes
`timeliness.max_reporting_delay_hours` (default `24`, shared with the
EMIR equivalent). Other checks are parameter-free.

## Roadmap

Future SFTR checks (require additional historical or schema context):

- Repo rate / lending fee plausibility (bounds, sign)
- Reuse indicator coherence vs. observed collateral reuse
- Collateral haircut Luhn check (ISIN check digit)
- Cross-record consistency: MODI without prior NEWT, ETRM before NEWT
- Margin lending–specific fields

## Adding a check

Each check lives under `crates/opendqi-core/src/dq/sftr/`. Implement
the `SftrCheck` trait, add a positive + negative unit test, then
register the type in `default_sftr_checks()` in
`crates/opendqi-core/src/dq/mod.rs`.
