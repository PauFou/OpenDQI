# OpenDQI Python / Arrow bindings — architecture spec for v0.12

> **Status: IMPLEMENTED in v0.12.0.** P0 → P5 of this spec all
> landed on `main` in the v0.12.0 chantier (commits `835b064`,
> `26f5fc3`, `31812a8`, `54bbf52`, `2d0a0d4`, P5 commit). The
> crate `crates/opendqi-py/` is live, builds via maturin, ships
> abi3-py39 wheels via `.github/workflows/python-release.yml`,
> and exposes the API documented below verbatim.
>
> This document is preserved as-is for historical reference and
> as the authoritative architecture description. See
> `CHANGELOG.md` for the as-shipped notes.

## Why

OpenDQI today is a Rust binary + CLI + local web UI. Every line of
business logic lives in `opendqi-core` and is exercised by 216
deterministic data-quality checks against 12 ISO 20022 messages.
The product is technically mature (216 checks, 762 tests, golden
suite, XSD-conformance gate, streaming pipeline with a measured
~32 % EMIR-1M peak reduction post-M0.27).

The risk now is **not technical, it is integration**. A regulatory-
reporting data team running on Databricks, Airflow or a local
DuckDB notebook cannot easily call OpenDQI today. They have to
shell out, parse files, materialise CSV results, then re-ingest.
That cost-of-integration is the single biggest adoption blocker.

The DuckDB analogy is the right reference: DuckDB succeeded
because it was *embeddable* — `import duckdb` from Python, then
talk to it the way you talk to your DataFrame. OpenDQI should
become **the DuckDB of EMIR/SFTR data quality** — local, fast,
embeddable, scriptable, useful before there is any platform
around it.

## Scope strict — v0.12.0 only

**In scope (must ship):**

```python
import opendqi

# Path-based: read a normalized Parquet, run checks, get Arrow back
result = opendqi.emir.scan_parquet("tsr.parquet")
result.summary       # dict
result.issues        # pyarrow.Table  (~13 columns, see § DqIssue schema)
result.normalized    # pyarrow.Table | None

# Table-based: skip the file roundtrip, pass an Arrow table directly
result = opendqi.emir.scan_table(
    pa_table,
    mapping={
        "uti":                   "UTI",
        "valuation_timestamp":   "ValuationTimestamp",
        "maturity_date":         "MaturityDate",
        # ... canonical field → user column
    },
    profile="tr_state",  # or "scan" (default), "feedback", etc.
)
```

Symmetric `opendqi.sftr.*` surface (same two functions).

A third helper for the parsers, optional in v0.12 :

```python
records = opendqi.emir.parse_xml("auth107.xml")   # pyarrow.Table
```

**Non-goals (deferred, do NOT scope-creep into v0.12):**

- Native Spark UDFs (PySpark `pandas_udf` wrapping works out of the
  box if `scan_table` accepts an Arrow table — but no dedicated
  `opendqi.spark` namespace).
- Magic DataFrame integration for *every* pandas/Polars version.
  Users construct an Arrow table themselves; we provide one entry
  point and document the 5-line Polars / pandas / DuckDB recipes.
- Multi-regime in a single call (`opendqi.scan(...)` auto-detecting
  EMIR vs SFTR). The CLI doesn't do it; the Python layer matches.
- The local web UI from Python. The Axum server stays Rust-only;
  Python users embed the engine, not the UI.
- A managed-cloud / SaaS offering. The whole point is "run it in
  your VPC, your Databricks, your Airflow". SaaS is a v1.x+
  conversation.

## Module layout

A new workspace crate, **independent of the existing 7** (zero
modification of `opendqi-core` / `opendqi-xml` / `opendqi-io` /
`opendqi-report` / `opendqi-store` / `opendqi-cli` /
`opendqi-server`). Built by [maturin](https://github.com/PyO3/maturin),
not by `cargo build` from the workspace root, so MSRV / CI of the
existing crates is unaffected.

```
crates/opendqi-py/
    Cargo.toml              # see § Deps freeze below
    pyproject.toml          # maturin build backend
    src/
        lib.rs              # #[pymodule] opendqi
        emir.rs             # opendqi.emir.{scan_parquet, scan_table, parse_xml}
        sftr.rs             # symmetric
        convert.rs          # EmirRecord/SftrRecord ↔ Arrow RecordBatch (in/out)
        issues.rs           # Vec<DqIssue> → Arrow Table  (the v1.0 contract)
        result.rs           # PyScanResult { summary, issues, normalized }
        errors.rs           # anyhow::Error → PyErr (mapped to opendqi.OpenDQIError)
    python/
        opendqi/
            __init__.py     # re-exports the #[pymodule] entry points
            _typing.py      # TypedDict for summary, Mapping for scan_table
            py.typed
    tests/
        test_emir.py        # uses examples/quickstart-emir/ from the repo
        test_sftr.py
        test_issues_schema.py  # locks the DqIssue Arrow schema (= v1.0 contract)
    README.md
```

## Reuse of the existing Rust core

`opendqi-py` is a **thin wrapper** — no parser, no check, no
report builder is reimplemented. It composes :

| Need | Already in | New in `opendqi-py` |
|---|---|---|
| Parse `auth.107.xml` → `Vec<TrStateRecord>` | `opendqi_xml::read_emir_tr_state_xml` | — |
| Parse `auth.092.xml` → `Vec<FeedbackRecord>` | `opendqi_xml::read_emir_feedback_xml` | — |
| Parse 10 other ISO 20022 messages | `opendqi-xml/src/*.rs` | — |
| Read Parquet → `Vec<EmirRecord>` | `opendqi_io::read_emir_parquet` (already exists) | — |
| Run all single-batch checks | `opendqi_core::dq::default_checks()` | — |
| Online summary | `opendqi_core::IssueAggregator` (M0.21) | — |
| Streaming issue sink | `opendqi_core::SortedIssueSink` (M0.22) | — |
| `EmirRecord` → Arrow `RecordBatch` | `opendqi-io/src/parquet_out.rs::build_emir_batch` (private today, needs `pub`) | trivial visibility bump |
| `DqIssue` → Arrow `RecordBatch` | **does not exist yet** — built once here | new schema (see below) |
| PyO3 Python <-> Rust glue | — | yes (the whole point of the crate) |
| Arrow ↔ Python ABI | `arrow-pyarrow` (new dep) | yes |

The two new pieces are : (1) exposing one Arrow conversion that
already exists in the Parquet writer, and (2) defining the
`DqIssue → Arrow` schema (this becomes the v1.0 stable contract).

## Arrow schema mapping — `EmirRecord` (54 cols)

`EmirRecord` already maps to an Arrow `RecordBatch` inside the
Parquet writer (`opendqi-io/src/parquet_out.rs::build_emir_batch`).
The schema is the documented v1.0 contract for the Parquet output;
the Python `scan_table` accepts the **exact same** Arrow schema as
input, so the round-trip is lossless.

Field type conventions (locked, from `parquet_out.rs:30-32`) :

- `Option<Decimal>`        → `Decimal128(38, 10)` (28 integer + 10 fractional digits — covers regulatory notionals up to 10²⁸ and haircuts at 10⁻¹⁰)
- `Option<NaiveDate>`      → `Date32` (days since 1970-01-01)
- `Option<DateTime<Utc>>`  → `Timestamp(Microsecond, "UTC")`
- `Option<String>`         → `Utf8` (nullable)
- `Option<bool>`           → `Boolean` (nullable)
- `HashMap<String,String>` (`raw_fields` catch-all) → `Utf8` (JSON-serialised object)

The 54 columns are listed in [`docs/parquet-normalize.md`](parquet-normalize.md) §EMIR — they are
the existing Parquet schema, unchanged by this milestone.

For `scan_table(table, mapping=...)`, the `mapping` dict maps
**canonical Rust field name** (e.g. `valuation_timestamp`) to
**user column name** in the input Arrow table. Unmapped fields
default to `None` ; unknown columns in the input become
`raw_fields` entries. This is the same contract `opendqi emir scan
--mapping mapping.yml` uses for CSV input today (see
`crates/opendqi-io/src/csv_in.rs`).

## Arrow schema mapping — `SftrRecord` (31 cols)

Symmetric. The 31 columns are listed in
[`docs/parquet-normalize.md`](parquet-normalize.md) §SFTR, with the
SFTR-specific fields (`loan_value`, `collateral_value`, `haircut`,
`sft_type`, `master_agreement_type/version`, `settlement_date`,
`reuse_indicator`, `security_identifier`, etc.).

## Arrow schema — `DqIssue` (the v1.0 contract)

**This is the deliverable that locks the v1.0 output contract.**
Once shipped in v0.12, any future change is breaking.

| col | type | nullable | source |
|---|---|---|---|
| `check_id` | Utf8 | no | `DqIssue::check_id` |
| `regime` | Utf8 | no | `Regime` (`"emir"` / `"sftr"`) |
| `severity` | Utf8 | no | `Severity` (`"info"` / `"warning"` / `"high"` / `"critical"`) |
| `dimension` | Utf8 | no | `DqDimension` (6 values) |
| `record_id` | Utf8 | yes | `DqIssue::record_id` |
| `uti` | Utf8 | yes | `DqIssue::uti` |
| `field` | Utf8 | yes | `DqIssue::field` |
| `value` | Utf8 | yes | `DqIssue::value` |
| `message` | Utf8 | no | `DqIssue::message` |
| `evidence_json` | Utf8 | yes | `serde_json::to_string(&DqIssue::evidence)` |
| `source_file` | Utf8 | yes | `DqIssue::source_file` |

(Severity / regime / dimension stay `Utf8` rather than dictionary-
encoded for v0.12 simplicity. We can add a `dictionary_encoded`
flag later — non-breaking.)

This matches **exactly** the columns of the existing CSV output
(`crates/opendqi-report/src/csv_out.rs::write_issues_csv_from_iter`),
so a Python user can `pyarrow.csv.read_csv(issues.csv)` and get a
schema-identical table. That parity is the contract.

## `summary` Python type

Returns the existing `ScanSummary` (M0.21 `IssueAggregator::into_summary`)
as a Python `dict`. Fields :

```python
{
    "regime": "emir" | "sftr",
    "files_processed": int,
    "records_processed": int,
    "issues_total": int,
    "issues_by_severity": {"info": int, "warning": int, "high": int, "critical": int},
    "issues_by_dimension": {"completeness": int, "validity": int, "accuracy": int,
                            "consistency": int, "uniqueness": int, "timeliness": int},
    "quality_score": float,
    "started_at": str (ISO 8601),
    "finished_at": str (ISO 8601),
}
```

Identical to the on-disk `summary.json` — same arithmetic
(M0.21 `quality_score_from_counts`, scoring.rs:33). Goldens lock
this in CI from v0.10.0.

## Build — maturin

```bash
pip install maturin
cd crates/opendqi-py
maturin develop          # local install into the active venv
maturin build --release  # produces a wheel under target/wheels/
```

`pyproject.toml` declares `requires-python = ">=3.9"` and uses
**abi3** so a single wheel covers 3.9+ on each target — keeps the
release matrix at 4 wheels per version, not 4 × N Python minors.

CI matrix (added in a new `.github/workflows/python-release.yml`,
generated by `maturin-action`) :

| Target | manylinux base | macOS min |
|---|---|---|
| `x86_64-unknown-linux-gnu` | manylinux2014 | — |
| `aarch64-unknown-linux-gnu` | manylinux2014 | — |
| `x86_64-apple-darwin` | — | 11.0 |
| `aarch64-apple-darwin` | — | 11.0 |

These are the same 4 targets as the `cargo-dist` CLI release
workflow added in v0.11.0. Windows is deferred (matches the
existing CI matrix Ubuntu + macOS only).

`maturin publish` to PyPI is gated on the same explicit-user-ask
contract as `git push --tags` today (CLAUDE.md release hygiene).

## Deps freeze

The single hardest-to-debug pitfall is **Arrow ABI mismatch**
between the Rust core and `arrow-pyarrow`. They share a C ABI
that breaks across major versions.

```toml
# crates/opendqi-py/Cargo.toml
[dependencies]
opendqi-core   = { path = "../opendqi-core" }
opendqi-xml    = { path = "../opendqi-xml" }
opendqi-io     = { path = "../opendqi-io" }
opendqi-report = { path = "../opendqi-report" }

# MUST match workspace `arrow-array = "53"` (Cargo.toml:38) :
arrow-array    = "53"
arrow-schema   = "53"
arrow-pyarrow  = "53"

pyo3           = { version = "0.22", features = ["extension-module", "abi3-py39"] }
serde_json     = { workspace = true }
anyhow         = { workspace = true }
```

The CI on PR adds **one assertion**: that `arrow-pyarrow`'s version
matches `arrow-array`'s. A simple `cargo tree -p opendqi-py | grep
arrow- | awk` + `grep` is enough.

## v0.12 milestone breakdown — 5 increments

Each increment ships in its own commit, behind a single feature
branch, with the same discipline as the v0.10.0 streaming
roll-out :

- **P1 — crate squelette + maturin CI.** Empty `#[pymodule]`,
  `pyproject.toml`, `maturin develop` works locally, CI matrix
  green on the 4 targets but produces only an empty wheel. ~200
  LOC, no functional code, no test surface. Locks the build
  contract before any logic.
- **P2 — `scan_parquet(path) → summary` (read-only).** First real
  call : read the Parquet via `opendqi_io::read_emir_parquet`,
  run `default_checks()`, return only the summary dict (no Arrow
  yet). Tests use `examples/quickstart-emir/` fixtures from this
  repo. Locks the file-path entry point.
- **P3 — `DqIssue → Arrow Table` + `result.issues`.** Implement
  the schema above, add `result.issues` to the return value,
  schema-pinned test (loads `pyarrow.csv.read_csv` of the existing
  `issues.csv` golden and asserts column-by-column type equality).
  **This is the v1.0 contract** — review and freeze before P4.
- **P4 — `scan_table(arrow_tbl, mapping)`.** Add the Arrow-in path
  using `arrow-pyarrow` to receive the table without copy.
  Implement the mapping → `EmirRecord` projection via the same
  helper the existing `csv_in.rs` uses. Tests cover the round-trip
  `Parquet → pyarrow → scan_table` vs `scan_parquet` (must match
  byte-for-byte on summary + issues).
- **P5 — `result.normalized` + release.** Optional `normalize=True`
  flag returns the EmirRecord-as-Arrow batch alongside issues
  (zero-copy via the same Arrow IPC export). Release v0.12.0 :
  4 wheels uploaded to PyPI, doc page, README quickstart block.

Each increment must keep the existing 762 tests + 19 goldens
byte-identical (zero touch on `opendqi-core` / `-xml` / `-io` /
`-report`).

## Out of scope, deferred to v0.13+

- Multi-file batch from Python (`scan_directory`).
- Async iterator over issues (streaming back to Python without
  materialising the full table) — needs the M0.22 sink's iterator
  exposed through Arrow C Data Interface, non-trivial.
- A Polars `LazyFrame`-aware fast path that pushes column
  selection down into Rust.
- The post-TR cross-message subcommands (`tr-audit`,
  `collateral-audit`, `book-reconcile`) — those take multiple
  files / multiple regimes and need a dedicated multi-table API.
  Trivial follow-on once P4 is in place.
- A Python-side `opendqi.feedback` module wrapping the SQLite
  history store. (The store is opt-in; the bindings should not
  require it.)

## Verification (at v0.12 ship time)

1. `pip install opendqi==0.12.0` from PyPI on a clean Python 3.10
   venv on each of the 4 targets succeeds with no compile step.
2. The README quickstart block runs end-to-end on the
   `examples/quickstart-emir/auth107-tsr.xml` fixture (via a
   one-liner that calls `opendqi.emir.parse_xml` then
   `opendqi.emir.scan_table`) and produces a summary equal to the
   existing CLI golden.
3. The `result.issues` Arrow table columns + types are
   byte-identical to `pyarrow.csv.read_csv(issues.csv)` of the
   matching golden CSV.
4. `arrow-array` and `arrow-pyarrow` major versions match in the
   shipped wheel's `METADATA`.

## Hors-scope explicite v0.11.0

This document is delivered in **v0.11.0**. No Python code, no new
crate, no maturin, no PyO3 dep. The v0.11.0 adoption pack ships
*the plan*. The implementation begins in v0.12.0 on a dedicated
feature branch, gated on explicit user request as every release
since v0.1.0 has been.
