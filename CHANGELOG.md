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
