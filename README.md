# OpenDQI

OpenDQI is a local-first data quality engine for EMIR and SFTR regulatory reporting files.

It helps regulatory reporting, compliance, operations, and data teams validate file structure, normalize reporting data, detect data quality issues, and generate actionable reports before or after submission to a Trade Repository.

## Features

- EMIR reporting file scanning
- SFTR support planned
- XML well-formedness checks (planned)
- XSD validation integration (planned)
- CSV input with configurable mapping
- **21 EMIR data-quality checks** (16 aligned with the official ESMA EMIR Refit Validation Rules) covering completeness, validity, accuracy, uniqueness, timeliness and consistency — see [`docs/emir-checks.md`](docs/emir-checks.md)
- HTML, JSON, and CSV outputs
- CLI batch mode
- Local web UI with drag-and-drop (planned)
- Runs locally by default

## Why OpenDQI?

Trade Repositories can validate and process regulatory reports, but firms often need independent controls over data quality, timeliness, completeness, consistency, and accepted-but-risky reports.

OpenDQI focuses on local, transparent, reproducible data quality scanning.

## Quick start

Build from source (requires Rust stable):

```bash
cargo build --release
```

Scan a CSV file against the EMIR canonical model:

```bash
./target/release/opendqi emir scan ./examples/emir/sample.csv \
  --mapping ./examples/emir/sample_mapping.yml \
  --out ./report
```

Outputs:

```text
report/report.html
report/summary.json
report/issues.csv
```

## Example checks

OpenDQI can detect issues such as:

- missing UTI
- duplicate UTI
- missing valuation
- abnormal maturity dates (including placeholder dates like 2099-12-31)
- late reporting
- lifecycle inconsistencies (planned)
- valuation after termination (planned)
- inconsistent collateral fields (planned)

## Input formats

Supported:

- CSV with mapping YAML
- OpenDQI v0.1 simplified XML (see [`docs/xml-format.md`](docs/xml-format.md))
- Official **ISO 20022 `auth.030.001.03`** EMIR Refit XML (see [`docs/iso20022-emir.md`](docs/iso20022-emir.md))
- Directories of mixed CSV/XML files
- Optional XSD validation via `xmllint` (see [`docs/xsd-validation.md`](docs/xsd-validation.md))

Planned:

- SFTR `auth.108` ISO 20022 reports
- ZIP/GZIP archives
- Parquet
- Trade Repository feedback files

## Status

OpenDQI is in early development. The first supported regime is EMIR (CLI MVP). SFTR support and the local web UI are planned.

See [`CHANGELOG.md`](CHANGELOG.md) for release notes.

## Documentation

- CLI reference: `opendqi --help`, `opendqi emir --help`
- CSV mapping guide: see `examples/emir/sample_mapping.yml`

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md).

## Security

See [`SECURITY.md`](SECURITY.md) for the vulnerability disclosure process.

## Disclaimer

OpenDQI is not a Trade Repository, ARM, reporting gateway, or regulatory certification tool. It does not submit reports. It provides local data quality analysis and validation support.

Users remain responsible for their regulatory reporting obligations and should validate outputs against applicable rules, internal controls, and professional advice.

## License

Licensed under the Apache License, Version 2.0. See [`LICENSE`](LICENSE).
