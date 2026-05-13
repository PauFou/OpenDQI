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
