# OpenDQI

[![CI](https://github.com/PauFou/OpenDQI/actions/workflows/ci.yml/badge.svg)](https://github.com/PauFou/OpenDQI/actions/workflows/ci.yml)
[![Security audit](https://github.com/PauFou/OpenDQI/actions/workflows/deny.yml/badge.svg)](https://github.com/PauFou/OpenDQI/actions/workflows/deny.yml)

**OpenDQI turns EMIR/SFTR Trade Repository activity, state, and rejection files into actionable data quality intelligence.**

A local-first engine that ingests both the reports a firm submits to its Trade Repository and the files the TR sends back, and converts them into reproducible HTML, JSON, CSV, and Parquet outputs.

## Why OpenDQI?

Trade Repositories process regulatory reports and return activity, state, rejection, and other feedback files. These files are essential, but they are not always easy to analyse operationally.

OpenDQI turns raw EMIR/SFTR submissions and TR feedback files into actionable data quality intelligence.

It helps teams answer:

- What did the TR accept?
- What did the TR reject?
- Which rejection causes are recurring?
- What is the current TR state?
- Which outstanding trades have stale or missing valuations?
- Which trades look active at the TR but should not be?
- Which accepted records still look risky from a data quality perspective?
- How does the TR state compare with internal booking data? *(planned)*

OpenDQI runs locally by default and produces reproducible HTML, JSON, CSV, and Parquet outputs.

## Product layers

OpenDQI organises its work around the three layers of a TR-firm conversation. See [`docs/positioning.md`](docs/positioning.md) for the full picture.

1. **Activity layer (TAR)** — what was submitted or processed during a period. Today: `opendqi emir scan` for firm submissions and `opendqi emir tr-activity-scan` for TR replays of `auth.030`, with 5 EMIR.TRA.* checks (repeated correction, NEWT/MODI/TERM spikes, duplicate NEWT in batch, TAR↔TSR coherence). See [`docs/tr-activity-checks.md`](docs/tr-activity-checks.md).
2. **State layer (TSR)** — what the TR currently believes is outstanding. Today: `opendqi emir tr-state-scan <auth.107>` with 7 state-health checks (outstanding-summary, stale / missing valuation, active-past-maturity, placeholder-date, duplicate-active-UTI, valuation-after-termination). Outputs `tr_state_issues.csv` / `tr_state_report.html`, distinct from the activity / feedback layers. See [`docs/tr-state-checks.md`](docs/tr-state-checks.md).
3. **Rejection layer** — what failed and why. Today: `opendqi emir feedback <auth.092>` plus the top-level `opendqi feedback list/resolve/stale/analytics` workflow over the local SQLite store. The `analytics` action surfaces top rejection causes, repeated rejected UTIs, age buckets, rejected-then-accepted detection, and exports a `rejection_profile.yml`. See [`docs/rejection-analytics.md`](docs/rejection-analytics.md).

The auth.* message catalog and our parser coverage are tracked in [`docs/auth-messages.md`](docs/auth-messages.md).

## Features

- **EMIR and SFTR submission scanning** (`opendqi {emir,sftr} scan`) over CSV (with YAML mapping) or ISO 20022 XML (`auth.030.001.03` for EMIR, `auth.052.001.02` for SFTR), with optional `--xsd` validation via `xmllint`.
- **TR feedback ingestion** (`opendqi emir feedback`) over `auth.092` files, cross-referenced against the local history store, with a top-level Open/Resolved/Stale workflow (`opendqi feedback list/resolve/stale`). EMIR-only — SFTR has no rejection-feedback message (real `auth.080` is a reconciliation status advice → `opendqi sftr reconcile`).
- **Local SQLite history store** (opt-in via `--store`) that persists submissions, feedback rows, and reconciliation rows, and enables cross-batch lifecycle checks (MODI/ETRM without a prior NEWT, duplicate NEWT, valuation regression / after-termination across scans).
- **200 reproducible data-quality checks** today across EMIR (140) and SFTR (60). Layered as single-batch validity / completeness / consistency / accuracy / uniqueness / timeliness, cross-batch lifecycle, MAR/MSR margin reports, TSR / TAR / reconciliation / book-vs-TSR, EMIR `auth.092` feedback, the `EMIR.RST.*` reconciliation statistics family (auth.091), the `EMIR.WRN.*` data-quality warnings family (auth.106), and the **`EMIR.PSC.*` + `SFTR.PSC.*` pre-submission families driven by post-TR rejection patterns**. Catalogues: [`docs/emir-checks.md`](docs/emir-checks.md), [`docs/sftr-checks.md`](docs/sftr-checks.md), [`docs/lifecycle-checks.md`](docs/lifecycle-checks.md), [`docs/feedback-checks.md`](docs/feedback-checks.md), [`docs/reconciliation-checks.md`](docs/reconciliation-checks.md), [`docs/emir-recon-stats.md`](docs/emir-recon-stats.md), [`docs/pre-submission-checks.md`](docs/pre-submission-checks.md).
- **Deterministic outputs**: `summary.json`, `issues.csv`, `report.html`, and dedicated artefacts per layer (e.g. `tr_state_issues.csv` for TSR scans).
- **Runs locally by default**; no network calls, no cloud dependency, no SWIFT-licensed XSD redistribution.

## Quick start

Build from source (requires Rust stable):

```bash
cargo build --release
```

Scan a firm's EMIR submission (CSV):

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

Open a history store and ingest a TR rejection file:

```bash
opendqi emir scan ./reports/april/ --mapping ./mapping.yml \
  --store ./opendqi-history.db --out ./report-april/

opendqi emir feedback ./trade-repo/april/auth092.xml \
  --store ./opendqi-history.db --out ./feedback-april/

opendqi feedback list --store ./opendqi-history.db --status open
opendqi feedback resolve --store ./opendqi-history.db --uti <UTI>
```

## Input formats

Supported today:

- CSV with mapping YAML (EMIR).
- ISO 20022 `auth.030.001.03` (EMIR TAR submissions) — see [`docs/iso20022-emir.md`](docs/iso20022-emir.md).
- ISO 20022 `auth.052.001.02` (SFTR submissions) — see [`docs/iso20022-sftr.md`](docs/iso20022-sftr.md).
- ISO 20022 `auth.092` (EMIR TR rejection feedback to firm). SFTR `auth.080` is a reconciliation status advice, parsed via `opendqi sftr reconcile` (not feedback).
- A matching-style placeholder for `auth.106` / `auth.083` (see naming caveat in [`docs/auth-messages.md`](docs/auth-messages.md)).
- OpenDQI v0.1 simplified XML ([`docs/xml-format.md`](docs/xml-format.md)).
- Directories of mixed CSV/XML files.
- `.zip` archives (only `csv` / `xml` / `parquet` members are read;
  directory components in member names are ignored — no zip-slip) and
  single-stream `.gz` (e.g. `foo.csv.gz`), accepted wherever a scan
  command takes a file path. Extraction is to a per-run temp directory,
  reclaimed by the OS on reboot (same contract as `opendqi desktop`).
- Optional XSD validation via `xmllint` ([`docs/xsd-validation.md`](docs/xsd-validation.md)).

Planned:

- ISO 20022 `auth.107` (EMIR TSR — Phase 1).
- TR-output mode for `auth.030` (Phase 2).
- ISO 20022 `auth.079` (SFTR TSR — Phase 6).
- Parquet output **and input** — shipped, round-trip complete. `opendqi emir scan path/to/normalized.parquet --out report/` accepts Parquet alongside CSV / XML. See [`docs/parquet-normalize.md`](docs/parquet-normalize.md) for the schema and downstream usage with DuckDB / Polars / PyArrow.
- Parallel check execution via `rayon` — see [`docs/performance.md`](docs/performance.md) for benchmark numbers (~400 k EMIR records/sec, ~1.2 M SFTR records/sec on commodity hardware).

## Roadmap

Public summary; see [`docs/positioning.md`](docs/positioning.md) for context.

- **Phase 0** — stabilise current engine, audit auth.* naming, pivot positioning. ✅
- **Phase 1** — EMIR TSR health (`tr-state-scan` over `auth.107`). ✅
- **Phase 2** — EMIR TAR activity intelligence (`tr-activity-scan`). ✅
- **Phase 3** — Rejection analytics (`feedback analytics`). ✅
- **Phase 4** — Combined `tr-audit` command. ✅
- **Phase 5** — Book vs TSR reconciliation (`book-reconcile`). ✅
- **Phase 6** — SFTR equivalent modules. ✅ (`opendqi sftr scan/reconcile/tr-state-scan/tr-activity-scan/tr-audit/book-reconcile/normalize` — 60 SFTR checks across submission, TSR, TAR, reconciliation, and book-vs-TSR layers, including margin-lending state/activity. SFTR has no feedback message — there is no `sftr feedback`.)
- **Phase 7** — Local web UI. ✅ (`opendqi desktop` opens http://127.0.0.1:7878 — see [`docs/desktop-web-ui.md`](docs/desktop-web-ui.md).)
- **Phase 8** — EMIR `auth.091` reconciliation statistics (`recon-stats`) ✅, canonical-model completeness (`source_system`, `security_identifier`, `evidence`, `severity_overrides`) ✅.

## Status

**v0.3.0.** EMIR coverage spans submissions, TSR (`auth.107`), TAR (`auth.030`), feedback (`auth.092`), reconciliation (`auth.106`), MAR (`auth.108`), MSR (`auth.109`), and reconciliation statistics (`auth.091`). SFTR coverage spans submissions (`auth.052`), TSR (`auth.079`), TAR replay, reconciliation status advice (`auth.080` via `sftr reconcile`), the synthetic `auth.083` matching layer, and book-vs-TSR. SFTR has no rejection-feedback message. The EMIR `auth.107`/`auth.108`/`auth.109`/`auth.091`/`auth.092` and SFTR `auth.079`/`auth.080` TR feedback/state/reconciliation parsers are **schema-verified against the real ESMA ISO 20022 XSDs (documented subset)** — per-message coverage notes (extracted fields, ignored branches, honest limits) under [`docs/auth-messages/`](docs/auth-messages/); SWIFT-licensed XSDs are not redistributed. Margin lending (MGLD) integrated into TAR/TSR layers. 195 data-quality checks, 657 tests, the post-TR → pre-TR rejection-profile loop, email notifications on every report-producing command, structured evidence rendered in the HTML report, a **10-operation local web UI** (full CLI parity), and shell completions / man page.

- **CI** — `cargo fmt --check`, `cargo clippy -D warnings`, `cargo build`, `cargo test --workspace` on Ubuntu + macOS for every push and PR to `main`.
- **MSRV** — verified in CI: the workspace builds on Rust **1.87.0** (`cargo check --all-targets --locked`), kept in sync with `rust-version` in `Cargo.toml`.
- **Coverage** — `cargo-llvm-cov` reports line coverage on every push/PR; surfaced on the run summary (informational, non-gating).
- **Security audit** — `cargo-deny` runs `check advisories bans licenses sources` daily and on every push/PR.
- **Run the same checks locally before pushing**: `./scripts/preflight.sh` (one-shot setup: `cargo install cargo-deny --locked`).
- **Auto-run on every push**: `./scripts/install-hooks.sh` installs the pre-push git hook.

See [`CHANGELOG.md`](CHANGELOG.md) for release notes.

## Shell completions & man page

```bash
# Bash (system-wide)
opendqi completions bash | sudo tee /etc/bash_completion.d/opendqi

# Zsh (into a dir on $fpath)
opendqi completions zsh > ~/.zfunc/_opendqi

# Fish
opendqi completions fish > ~/.config/fish/completions/opendqi.fish

# Man page
opendqi man > opendqi.1 && man ./opendqi.1
```

Supported shells: `bash`, `zsh`, `fish`, `powershell`, `elvish`.

## Documentation

- Positioning & roadmap: [`docs/positioning.md`](docs/positioning.md).
- auth.* message catalog: [`docs/auth-messages.md`](docs/auth-messages.md).
- CLI reference: `opendqi --help`, `opendqi emir --help`.
- CSV mapping guide: see `examples/emir/sample_mapping.yml`.

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md).

## Security

See [`SECURITY.md`](SECURITY.md) for the vulnerability disclosure process.

## Disclaimer

OpenDQI is not a Trade Repository, ARM, reporting gateway, or regulatory certification tool. It does not submit reports. It provides local data quality analysis and validation support.

Users remain responsible for their regulatory reporting obligations and should validate outputs against applicable rules, internal controls, and professional advice.

## License

Licensed under the Apache License, Version 2.0. See [`LICENSE`](LICENSE).
