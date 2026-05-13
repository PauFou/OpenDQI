# SFTR data-quality checks

OpenDQI ships **40 single-batch SFTR checks** (this document), plus
SFTR post-TR intelligence layers — `auth.080` feedback,
`auth.083` reconciliation, `auth.079` TSR, `auth.052` TAR,
`auth.052 + auth.079 + auth.080` audit, and CSV-vs-TSR
book-reconcile. The single-batch catalog below covers
`opendqi sftr scan`; see
[`tr-state-checks.md`](tr-state-checks.md),
[`tr-activity-checks.md`](tr-activity-checks.md),
[`tr-audit.md`](tr-audit.md),
[`tr-feedback.md`](tr-feedback.md),
[`tr-reconciliation.md`](tr-reconciliation.md), and
[`book-reconcile.md`](book-reconcile.md) for the post-TR layers.

The catalog mirrors the EMIR check coverage — same dimensions,
similar logic, SFTR-specific field semantics (loan / collateral /
haircut / settlement, SFT type, master agreement).

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
| `SFTR.COMP.SFT_TYPE_MISSING` | Completeness | High | SFT type (REPO / BSB / SLEB / MGLD) absent |
| `SFTR.VLD.LEI_FORMAT_RC` | Validity | High | Reporting counterparty LEI not ISO 17442 shape |
| `SFTR.VLD.LEI_FORMAT_OC` | Validity | High | Other counterparty LEI not ISO 17442 shape |
| `SFTR.VLD.LEI_FORMAT_ERR` | Validity | High | Entity-responsible-for-reporting LEI not ISO 17442 shape |
| `SFTR.VLD.CURRENCY_LOAN` | Validity | Warning | Loan currency not three uppercase letters |
| `SFTR.VLD.CURRENCY_COLLATERAL` | Validity | Warning | Collateral currency not three uppercase letters |
| `SFTR.VLD.ISIN_COLLATERAL` | Validity | Warning | Collateral ISIN not ISO 6166 shape (2 letters + 9 alphanumeric + 1 digit) |
| `SFTR.VLD.SFT_TYPE_ENUM` | Validity | High | SFT type outside `{REPO, BSB, SLEB, MGLD}` |
| `SFTR.VLD.ACTION_TYPE_ENUM` | Validity | High | Action type not in standard SFTR codes |
| `SFTR.VLD.LOAN_PRECISION` | Validity | Warning | Loan value exceeds ESMA `decimal:18.5` precision |
| `SFTR.VLD.COLLATERAL_PRECISION` | Validity | Warning | Collateral value exceeds ESMA `decimal:18.5` precision |
| `SFTR.VLD.HAIRCUT_PRECISION` | Validity | Warning | Haircut exceeds ESMA `decimal:11.10` precision |
| `SFTR.VLD.RATE_PRECISION` | Validity | Warning | Rebate rate or lending fee exceeds ESMA `decimal:11.10` precision |
| `SFTR.VLD.MASTER_AGREEMENT_VERSION_FORMAT` | Validity | Warning | Master agreement version is not a 4-digit year |
| `SFTR.VLD.GMRA_GMSLA_VERSION_PLAUSIBLE` | Validity | Warning | GMRA / GMSLA version is not a publicly known edition |
| `SFTR.ACC.NEGATIVE_LOAN` | Accuracy | High | Loan value is negative |
| `SFTR.ACC.NEGATIVE_COLLATERAL` | Accuracy | High | Collateral value is negative |
| `SFTR.ACC.HAIRCUT_OUT_OF_RANGE` | Accuracy | Warning | Haircut < 0 or > 1.0 |
| `SFTR.UNI.DUPLICATE_UTI` | Uniqueness | Critical | Same UTI on multiple active SFT records |
| `SFTR.TIM.LATE_REPORTING` | Timeliness | High | Reporting delay above the configured threshold |
| `SFTR.CON.SETTLEMENT_BEFORE_EXECUTION` | Consistency | High | Settlement date precedes execution timestamp |
| `SFTR.CON.MATURITY_BEFORE_EFFECTIVE` | Consistency | High | Maturity date precedes effective date |
| `SFTR.CON.SELF_DEALING` | Consistency | High | Counterparty 1 equals counterparty 2 |
| `SFTR.CON.LOAN_NEEDS_CURRENCY` | Consistency | High | Loan value is reported without a currency |
| `SFTR.CON.COLL_NEEDS_CURRENCY` | Consistency | High | Collateral value is reported without a currency |
| `SFTR.CON.LOAN_COLL_CURRENCY_MISMATCH` | Consistency | Warning | Loan and collateral currencies differ |
| `SFTR.CON.REBATE_REQUIRES_REPO_OR_BSB` | Consistency | Warning | Rebate rate reported on a non-REPO / non-BSB SFT |
| `SFTR.CON.LENDING_FEE_REQUIRES_SLEB` | Consistency | Warning | Lending fee reported on a non-SLEB SFT |
| `SFTR.CON.NEWT_FORBIDS_PRIOR_UTI` | Consistency | Warning | NEWT action carries a prior UTI |
| `SFTR.CON.NEWT_FORBIDS_TERMINATION_DATE` | Consistency | Warning | NEWT action carries a termination date |
| `SFTR.CON.ETRM_REQUIRES_TERMINATION_DATE` | Consistency | High | ETRM action lacks a termination date |
| `SFTR.CON.COLU_REQUIRES_PORTFOLIO` | Consistency | High | COLU action lacks a collateral portfolio code |
| `SFTR.CON.REUU_REQUIRES_REUSE_INDICATOR` | Consistency | High | REUU action lacks the reuse indicator |

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
