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
