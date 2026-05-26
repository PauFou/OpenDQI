# opendqi (Python / Arrow bindings)

Python bindings to the [OpenDQI](https://github.com/PauFou/OpenDQI)
EMIR/SFTR data-quality engine. Wraps the Rust core in a small PyO3
+ Arrow C Data Interface layer; **the engine itself is not
reimplemented** — every parser, every check, the streaming sink
and the canonical record model are reused as-is.

## Status

**Preview (v0.12.x).** See
[`../../docs/python-roadmap.md`](../../docs/python-roadmap.md) for
the milestone breakdown. P1 ships the package skeleton (no
functions yet) ; P2–P5 add `scan_parquet`, `DqIssue → pyarrow.
Table` (the v1.0 contract), `scan_table(arrow_tbl, mapping)`,
`parse_xml`, and the `result.normalized` Arrow output.

## Install (when v0.12.0 is released)

```bash
# From the GitHub Release tarballs:
pip install https://github.com/PauFou/OpenDQI/releases/download/v0.12.0/opendqi-0.12.0-cp39-abi3-<target>.whl
```

PyPI publication is gated on a separate explicit-user-ask
release. For now the wheels live on the GitHub Releases page.

## Local development

```bash
# From the repo root, with Python 3.9–3.13 (3.14 supported as a
# runtime target via abi3-py39 but the build script of PyO3 0.22
# needs >= 3.13 — see the spec for details).
python3.12 -m venv crates/opendqi-py/.venv
source crates/opendqi-py/.venv/bin/activate
pip install --upgrade pip maturin pytest pyarrow

cd crates/opendqi-py
maturin develop                # build the cdylib + install in the venv
pytest tests/                  # run the smoke tests
python -c "import opendqi; print(opendqi.__version__)"
```

## Public API (target — surface grows commit-by-commit through P5)

```python
import opendqi

# P2 — path-based scan, summary only.
result = opendqi.emir.scan_parquet("tsr.parquet")
result.summary        # dict — same fields as summary.json

# P3 — DqIssue → pyarrow.Table (v1.0 stable schema, byte-equal to
# the existing issues.csv).
result.issues         # pyarrow.Table | None

# P4 — Arrow-in surface with a canonical-field → user-column mapping.
import pyarrow as pa
table = pa.table({...})
result = opendqi.emir.scan_table(table, mapping={
    "uti":                  "UTI",
    "valuation_timestamp":  "ValuationTimestamp",
})

# P4 (bonus) — parse any of the 12 ISO 20022 messages directly.
records = opendqi.emir.parse_xml("auth107.xml")  # pyarrow.Table

# P5 — optional normalized output (RecordBatch of the canonical model).
result = opendqi.emir.scan_parquet("tsr.parquet", normalize=True)
result.normalized     # pyarrow.Table | None
```

## License

From v0.19.0: **FSL-1.1-Apache-2.0** (source-available, converts
automatically to Apache-2.0 two years after each release tag) — same
licence as the parent [`OpenDQI`](https://github.com/PauFou/OpenDQI)
repository. See [`LICENSE.md`](LICENSE.md) (kept in sync with the
repo-root `../../LICENSE.md` by `scripts/release-license-bump.sh` on
every release commit) and [`../../COMPETING.md`](../../COMPETING.md).

Releases v0.1.0 through v0.18.0 on PyPI were shipped under Apache-2.0
and remain so.
