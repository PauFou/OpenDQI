# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Initial Cargo workspace with crates: `opendqi-core`, `opendqi-io`, `opendqi-report`, `opendqi-cli`, plus `opendqi-xml` and `opendqi-server` stubs.
- EMIR canonical record model and shared DQ issue / dimension / severity types.
- CSV ingestion with YAML field mapping.
- Five MVP EMIR checks: missing UTI, missing valuation, abnormal maturity, duplicate UTI, late reporting.
- `opendqi emir scan` CLI subcommand producing `summary.json`, `issues.csv`, and `report.html`.
- Synthetic example fixture under `examples/emir/`.
- EMIR XML ingestion using the simplified OpenDQI XML format v0.1 (`docs/xml-format.md`). `opendqi emir scan` now accepts `.xml` files and directories of mixed `.csv`/`.xml` inputs.
- Streaming XML well-formedness check via `quick-xml` and three format-level issue types: `EMIR.FMT.XML_NOT_WELLFORMED` (critical), `EMIR.FMT.XML_UNSUPPORTED_NAMESPACE` (warning), `EMIR.FMT.XML_UNKNOWN_ELEMENT` (info, de-duplicated per file).
- Fixtures `examples/emir/sample.xml` and `examples/emir/broken/malformed.xml`.

### Changed

- `--mapping` is now optional on `opendqi emir scan`; it is only required when the input set contains at least one CSV file.
- `discover_inputs` was renamed to `discover_emir_inputs` and accepts both `.csv` and `.xml` files.
- `opendqi emir scan` accepts an optional `--xsd <path>` flag. When set, every XML input is also validated against the schema and violations are surfaced as `EMIR.FMT.XSD_VIOLATION` issues (severity `high`). A dedicated `xsd_errors.csv` is written alongside the standard reports.
- `opendqi emir validate <input> --xsd <path>` is now a real command: it performs a streaming well-formedness check, runs XSD validation via `xmllint`, prints `path:line: message` to stderr for each violation, and exits non-zero when any issue is found.
- New `XsdValidator` trait in `opendqi-xml` with `ExternalXmllintValidator` (shells out to `xmllint --noout --schema`) and `NoopValidator` implementations.
- Canonical XSD `examples/emir/schemas/opendqi-emir-v0.1.xsd` and violation fixture `examples/emir/violates-schema.xml`.
- Documentation: new `docs/xsd-validation.md`.
- **ISO 20022 `auth.030.001.03` (EMIR Refit) ingestion adapter**. `opendqi emir scan` auto-detects the namespace and routes to the new extractor — no flag required. All 8 EMIR action types are recognised (`NEWT`/`MODI`/`CORR`/`ETRM`/`POSC`/`VALU`/`MARU`/`OTHR`); both legs of swap-like products are captured.
- `EmirRecord` extended with leg-2 notional, margin posted/collected pairs, clearing CCP LEI, master agreement metadata, intragroup / hedging indicators, valuation type, option greeks, and a `raw_fields: BTreeMap<String, String>` catch-all for any auth.030 leaf not in the typed-routing table.
- Synthetic fixture `examples/emir/iso20022/sample.xml` (10 trades, all 8 action types, hand-authored — not derived from ESMA/SWIFT examples).
- Documentation: new `docs/iso20022-emir.md` (mapping table + how to obtain the official SWIFT-licensed XSD).
- **16 new EMIR data-quality checks driven by the ESMA EMIR Refit Validation Rules**, bringing the catalog to 21 checks total. New checks: counterparty / entity LEI shape (3 checks), ISO 4217 currency shape (2 checks), counterparty / currency / valuation completeness (5 checks), zero / negative notional (2 checks), valuation-after-reporting timeliness, reporting-before-execution / cleared-requires-CCP / valuation-after-termination consistency. See [`docs/emir-checks.md`](docs/emir-checks.md) for the full catalog with ESMA-VR references.
- Synthetic fixture `examples/emir/extended-checks.csv` + `extended-checks.yml` exercising one positive case per new check.
- `opendqi-core/src/dq/formats.rs`: shared ISO 17442 LEI shape and ISO 4217 currency shape validators.
- `opendqi-io/src/csv_in.rs`: now ingests the `clearing_ccp_lei` column when present in the mapping.

### Changed

- Existing fixtures (`examples/emir/sample.csv`, `sample.xml`, `iso20022/sample.xml`) intentionally exhibit LEI shapes that end with `AA` and valuation timestamps later than reporting timestamps; the new checks correctly flag these as defects. Issue counts for those fixtures rise from 7/7/8 to ~38/38/47. This is expected and demonstrates the new coverage in action — the fixtures are not yet rewritten to be defect-free.

### Added (SFTR)

- **SFTR regime support**: new canonical `SftrRecord` struct, `SftrCheck` trait, `default_sftr_checks()` registry and `run_all_sftr` runner in `opendqi-core`.
- **ISO 20022 `auth.052.001.02` ingestion adapter** (`opendqi-xml::sftr`): namespace-aware routing, selective extractor with the same `raw_fields` catch-all pattern as the EMIR adapter. Recognises all SFTR action types (NEWT/MODI/CORR/ETRM/VALU/COLU/MARU/REUU/POSC/OTHR) and the four SFT wrappers (Repo/BSB/SLEB/MGLD).
- **5 MVP SFTR data-quality checks**: `SFTR.COMP.UTI_MISSING`, `SFTR.COMP.COLLATERAL_VALUE_MISSING`, `SFTR.COMP.HAIRCUT_MISSING`, `SFTR.TIM.LATE_REPORTING`, `SFTR.UNI.DUPLICATE_UTI`. See [`docs/sftr-checks.md`](docs/sftr-checks.md).
- **CLI**: `opendqi sftr scan <path> --out <dir>` is now a real command (previously a placeholder). XML-only ingestion for this milestone.
- Synthetic fixture `examples/sftr/iso20022/sample.xml` (10 SFTs hand-authored covering Repo / BSB / SLEB and all DQ patterns) plus integration test `crates/opendqi-xml/tests/sftr_integration.rs`.
- Documentation: new `docs/sftr-checks.md` and `docs/iso20022-sftr.md`.

### Added (SFTR parity with EMIR)

- **15 additional SFTR data-quality checks**, bringing the SFTR catalog from 5 to **20** (same dimension coverage as EMIR). Highlights: LEI shape on the three party LEIs, ISO 4217 shape on loan / collateral currencies, ISO 6166 shape on collateral ISIN, missing-counterparty checks, missing-currency checks, negative loan / collateral, haircut out-of-range, settlement-before-execution, maturity-before-effective.
- **`opendqi sftr scan --xsd <xsd>`** is now active: schema violations surface as `SFTR.FMT.XSD_VIOLATION` (high) issues, a dedicated `xsd_errors.csv` is written alongside the standard reports.
- **`opendqi sftr validate <input> --xsd <xsd>`** is a real command (well-formedness + XSD validation, exits non-zero on any defect).
- New ISIN shape validator `is_valid_isin` in `opendqi-core/src/dq/formats.rs`.
- Extended SFTR fixture `examples/sftr/iso20022/extended.xml` exercising one positive case per new check.

### Added (EMIR margin / clearing / enums / dates depth)

- **30 additional EMIR data-quality checks**, bringing the EMIR catalog from 21 to **51** (covers margin amounts, clearing metadata, enumerated codes, and a broader set of date / consistency rules).
  - Margin (8): negative initial/variation margin posted/collected, `MARU` action requires margin, FLCL requires initial/variation margin posted + collateral portfolio code.
  - Clearing (5): CCP LEI shape, NCLR forbids CCP, clearing status missing/enum, intragroup indicator missing.
  - Enumerations (8): `action_type` / `event_type` / `valuation_type` / `trading_capacity` / `asset_class` / `nature` / `master_agreement_type` / `collateralisation_category`.
  - Dates & consistency (9): event-before-execution, maturity-in-past, termination/effective after maturity, ETRM requires termination date, and four "missing" completeness checks (nature / trading capacity / master agreement / asset class).
- New `is_in` helper in `opendqi-core/src/dq/formats.rs` for case-insensitive small-enum matching.
- `opendqi-io/src/csv_in.rs` now ingests `nature`, `corporate_sector`, `trading_capacity`, `valuation_type`, `master_agreement_type`, `master_agreement_version`, `intragroup_indicator` columns when present in the mapping.
- Synthetic fixture `examples/emir/margin-and-enums.csv` + `margin-and-enums.yml`: 31 rows exercising each of the 30 new check_ids at least once.

### Changed (EMIR depth side effects)

- The pre-existing `examples/emir/extended-checks.csv` now produces ~87 issues (up from 17) because the new completeness/enum checks correctly flag many fields that fixture leaves unpopulated. This is expected — the fixture is preserved as-is so the additional coverage is visible.
