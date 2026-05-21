# OpenDQI

[![CI](https://github.com/PauFou/OpenDQI/actions/workflows/ci.yml/badge.svg)](https://github.com/PauFou/OpenDQI/actions/workflows/ci.yml)
[![Security audit](https://github.com/PauFou/OpenDQI/actions/workflows/deny.yml/badge.svg)](https://github.com/PauFou/OpenDQI/actions/workflows/deny.yml)

**Turn EMIR/SFTR Trade Repository files into deterministic data quality reports and Arrow tables — locally, reproducibly, from your existing data stack.**

A local-first Rust engine that ingests both the reports a firm submits to its Trade Repository and the files the TR sends back, then produces reproducible HTML, JSON, CSV, Parquet, and `pyarrow.Table` outputs. Use it from your terminal (CLI), your browser (local web UI on http://127.0.0.1:7878), or your Python / PyArrow notebook. No network calls. No cloud dependency. Embeds in your own pipeline.

## The three things OpenDQI does

Three workflows mirror the three layers of any TR/firm conversation. Each one is one command. Each one writes a deterministic HTML/CSV/JSON triple under `--out`.

### 1. TR state health — *"what does the TR think I have open?"*

```bash
opendqi emir tr-state-scan auth107.xml --out ./report/
```

Ingests the daily `auth.107` Trade State Report and surfaces stale valuations, trades past maturity, duplicate active UTIs, valuation after termination, placeholder maturity dates. **Score + 16 issues** on the shipped 8-record fixture.

### 2. Rejection intelligence — *"what's the TR throwing back, and why?"*

```bash
opendqi emir feedback auth092.xml --store ./history.db --out ./feedback/
opendqi feedback analytics --store ./history.db --regime emir --out ./rejection_profile.yml
opendqi emir scan next-batch.csv --mapping mapping.yml --rejection-profile ./rejection_profile.yml --out ./pre-flight/
```

`auth.092` rejections feed a SQLite history store + an Open/Resolved/Stale workflow, and roll up into a `rejection_profile.yml` that gates the *next* submission via the `EMIR.PSC.*` family. **The post-TR ↔ pre-TR feedback loop.**

### 3. Combined audit — *"one report for the committee"*

```bash
opendqi emir tr-audit --tar auth030.xml --tsr auth107.xml --feedback auth092.xml --out ./audit/
```

Three layers, one HTML, plus 3 cross-layer `EMIR.AUD.*` coherence checks (rejected-but-outstanding-in-TSR, MODI-without-prior-NEWT, TERM-but-still-outstanding). **251 issues** on the shipped 20-record audit fixture.

Operator scenarios for each workflow in [`docs/use-cases.md`](docs/use-cases.md). SFTR has the same surface (`opendqi sftr ...`) minus rejection feedback (`auth.080` is a reconciliation status advice, not feedback).

## 30-second demo

```bash
git clone https://github.com/PauFou/OpenDQI && cd OpenDQI
bash scripts/demo.sh
```

Runs all three workflows above against the synthetic kit at [`examples/quickstart-emir/`](examples/quickstart-emir/), drops three `report.html` files under `/tmp/opendqi-demo/`, and opens the consolidated audit report in your default browser. Builds a debug binary on the first run; subsequent runs are sub-second.

## Install

Pick the channel that matches how you work — same engine, same 216 checks, same v1.0 Arrow output contract behind each.

### Python (recommended for data teams)

```bash
pip install opendqi                # core: pyarrow only
pip install opendqi[spark]         # + pyspark>=3.5 (v0.14)
pip install opendqi[polars]        # + polars>=0.20 (v0.14)
pip install opendqi[all]           # + both
```

### CLI (recommended for ops / one-shot audits)

```bash
curl -sSL https://github.com/PauFou/OpenDQI/releases/download/v0.14.0/opendqi-cli-installer.sh | sh
```

Pre-built binaries for Linux x86_64 + ARM64 and macOS x86_64 + ARM64 from the [GitHub Releases](https://github.com/PauFou/OpenDQI/releases) page.

### Rust source (build from a tag)

```bash
cargo install --git https://github.com/PauFou/OpenDQI --tag v0.14.0 opendqi-cli
```

### Then 5 lines that work after either install

```python
import opendqi
table = opendqi.emir.parse_xml("auth030.xml")
result = opendqi.emir.scan_table(table, mapping={c: c for c in table.column_names})
print(result.summary)   # dict — same shape as summary.json
print(result.issues)    # pyarrow.Table — v1.0 stable 11-column schema
```

Or one of the v0.13 cross-message workflows — 3 files, one call, post-TR audit including the 3 `EMIR.AUD.*` cross-layer coherence checks:

```python
result = opendqi.emir.tr_audit(
    tar="auth030.xml", tsr="auth107.xml", feedback="auth092.xml",
)
```

Or v0.14 native Spark — partition-friendly `mapInPandas`, stays distributed across executors :

```python
issues_sdf = opendqi.spark.scan_spark_dataframe(
    spark_df, regime="emir", mapping={"uti": "trade_uti", ...},
)   # returns a pyspark.sql.DataFrame of issues
```

Or **v0.15 Data Quality Pack** — 10 regulator-style indicators (numerator / denominator / rate / threshold / status) with drill-down evidence, granular issues co-produced :

```python
result = opendqi.emir.data_quality_pack(
    tsr="auth107.xml", tar="auth030.xml", feedback="auth092.xml",
)
print(result.indicators.to_pandas())   # 10 rows, v1.0 stable Arrow schema
result.report("./pack/")                # report.html + 4 CSV / JSON files
```

The full Python surface (v0.12→v0.15) adds **14 entry points**: `scan_parquet`/`scan_table`/`parse_xml` × 2 régimes + multi-file `scan_directory`/`scan_files` + cross-message `tr_audit`/`collateral_audit`/`book_reconcile`/`missing_collateral` + native data-platform `opendqi.spark.scan_spark_dataframe` + `opendqi.polars.scan_lazyframe` + **v0.15 `opendqi.emir.data_quality_pack`** (+ its EXPERIMENTAL `opendqi.spark.emir.data_quality_pack` wrapper). More on the Python side: minimal demo in [`examples/python/quickstart.py`](examples/python/quickstart.py) · 7 progressively-realistic patterns in [`examples/python/`](examples/python/) · executable Jupyter notebook in [`examples/python/quickstart.ipynb`](examples/python/quickstart.ipynb) · full quickstart guide in [`docs/python.md`](docs/python.md) · DQI spec in [`docs/data-quality-pack.md`](docs/data-quality-pack.md) · architecture spec in [`docs/python-roadmap.md`](docs/python-roadmap.md).

## Coverage at a glance

| | EMIR | SFTR | Total |
|---|---:|---:|---:|
| Single-batch DQ checks (6 dimensions) | 89 | 44 | 133 |
| TR-layer & cross-message (TSR · TAR · MAR · MSR · Recon · Warnings · Missing-collat · Audit · Collateral-audit · Book-vs-TR · Lifecycle · Feedback · Pre-submission) | 62 | 21 | 83 |
| **Live catalog (post-v0.10.0)** | **151** | **65** | **216** |

12 ISO 20022 messages parsed against the real ESMA XSD subset, gated locally by `OPENDQI_XSD_DIR` (SWIFT-licensed XSDs never redistributed). Full catalogues: [`docs/emir-checks.md`](docs/emir-checks.md) · [`docs/sftr-checks.md`](docs/sftr-checks.md). Per-message coverage notes: [`docs/auth-messages/`](docs/auth-messages/).

## Documentation

- **Get started** : [`docs/use-cases.md`](docs/use-cases.md) (operator scenarios) · [`examples/quickstart-emir/`](examples/quickstart-emir/) (3-file kit) · [`scripts/demo.sh`](scripts/demo.sh) (one-shot) · [`docs/python.md`](docs/python.md) (Python quickstart) · [`examples/python/`](examples/python/) (7 scripts + Jupyter notebook).
- **Data Quality Pack (v0.15)** : [`docs/data-quality-pack.md`](docs/data-quality-pack.md) — 10 regulator-style indicators above the 216 granular checks ; the "committee-readable" view.
- **Positioning** : [`docs/positioning.md`](docs/positioning.md) (3-layer mental model).
- **Per-workflow** : [`docs/tr-state-checks.md`](docs/tr-state-checks.md) · [`docs/tr-activity-checks.md`](docs/tr-activity-checks.md) · [`docs/tr-audit.md`](docs/tr-audit.md) · [`docs/tr-feedback.md`](docs/tr-feedback.md) · [`docs/rejection-analytics.md`](docs/rejection-analytics.md) · [`docs/pre-submission-checks.md`](docs/pre-submission-checks.md) · [`docs/book-reconcile.md`](docs/book-reconcile.md) · [`docs/collateral-audit.md`](docs/collateral-audit.md) · [`docs/emir-mar-msr.md`](docs/emir-mar-msr.md) · [`docs/emir-recon-stats.md`](docs/emir-recon-stats.md) · [`docs/emir-warnings.md`](docs/emir-warnings.md) · [`docs/sftr-missing-collateral.md`](docs/sftr-missing-collateral.md).
- **Engineering** : [`docs/auth-messages.md`](docs/auth-messages.md) · [`docs/iso20022-emir.md`](docs/iso20022-emir.md) · [`docs/iso20022-sftr.md`](docs/iso20022-sftr.md) · [`docs/xml-format.md`](docs/xml-format.md) · [`docs/xsd-validation.md`](docs/xsd-validation.md) · [`docs/parquet-normalize.md`](docs/parquet-normalize.md) · [`docs/history-store.md`](docs/history-store.md) · [`docs/lifecycle-cross-batch.md`](docs/lifecycle-cross-batch.md) · [`docs/desktop-web-ui.md`](docs/desktop-web-ui.md) · [`docs/email-notifications.md`](docs/email-notifications.md).
- **Reliability** : [`docs/reliability.md`](docs/reliability.md) · [`docs/performance.md`](docs/performance.md) · [`CHANGELOG.md`](CHANGELOG.md).
- **What's next** : [`docs/python-roadmap.md`](docs/python-roadmap.md) (Python/Arrow bindings architecture; v0.12 implemented, v0.13+ deferred).

## Input formats

CSV (with YAML mapping), ISO 20022 XML (12 supported messages — see above), directories of mixed files, `.zip` archives (no zip-slip — `csv`/`xml`/`parquet` members only, directory components dropped), single-stream `.gz`, and Parquet (read + write round-trip — same canonical schema as the bindings spec in [`docs/python-roadmap.md`](docs/python-roadmap.md)). Optional XSD validation via `xmllint`.

## Status & roadmap

| Version | Theme |
|---|---|
| **v0.15.0** (current) | **EMIR Data Quality Pack v1** — 10 regulator-style indicators above the 216 checks ; `opendqi emir data-quality-pack` CLI + `opendqi.emir.data_quality_pack` Python ; v1.0 stable Arrow schemas for indicators + evidence. See [`docs/data-quality-pack.md`](docs/data-quality-pack.md). |
| **v0.14.0** | Native Spark `mapInPandas` UDF + Polars LazyFrame fast path + `pip install opendqi[spark,polars,all]` |
| **v0.13.0** | Python feature expansion — 10 new entry points (multi-file + cross-message + Spark experimental) |
| **v0.12.0** | **Python/Arrow bindings preview** — `opendqi.emir.scan_parquet` + `scan_table(arrow_tbl, mapping)`, issues as `pyarrow.Table` |
| **v0.10.0** | Streaming-issue pipeline end-to-end (~32 % EMIR-1M peak RSS reduction, honest measurement on 3 views) + EMIR Article 11 collateral cross-reference (`COL.*`) |
| **v1.0.0** | Stable CLI / output / Arrow contract. Locked schemas. |

**MSRV** Rust 1.87.0 (verified in CI). **CI** `cargo fmt --check`, `cargo clippy -D warnings`, build, **831 tests** + **20/20 goldens** byte-identical, `cargo-deny` daily — all on Ubuntu + macOS. **Local preflight** : `./scripts/preflight.sh` (one-shot setup `cargo install cargo-deny --locked`). **Auto-run on push** : `./scripts/install-hooks.sh`.

Full release history in [`CHANGELOG.md`](CHANGELOG.md).

## Shell completions & man page

```bash
opendqi completions bash | sudo tee /etc/bash_completion.d/opendqi
opendqi completions zsh  > ~/.zfunc/_opendqi
opendqi completions fish > ~/.config/fish/completions/opendqi.fish
opendqi man > opendqi.1 && man ./opendqi.1
```

Supported shells: `bash`, `zsh`, `fish`, `powershell`, `elvish`.

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md).

## Security

See [`SECURITY.md`](SECURITY.md) for the vulnerability disclosure process. No SWIFT-licensed XSDs are committed; all fixtures are synthetic.

## Disclaimer

OpenDQI is not a Trade Repository, ARM, reporting gateway, or regulatory certification tool. It does not submit reports. It provides local data quality analysis and validation support.

**Specifically for the v0.15 Data Quality Pack**: the DQIs (`DQI_VAL_MISSING`, `DQI_REJ_RATE`, etc.) are **internal** data quality **indicators**, not regulatory verdicts. A `red` status is an internal alert asking the firm to investigate — not a declaration of non-compliance. The aggregation rates and the thresholds that drive them are firm-configurable. See [`docs/data-quality-pack.md`](docs/data-quality-pack.md#disclaimer--what-a-dqi-is-not) for the full vocabulary discipline.

Users remain responsible for their regulatory reporting obligations and should validate outputs against applicable rules, internal controls, and professional advice.

## License

Licensed under the Apache License, Version 2.0. See [`LICENSE`](LICENSE).
