# Reliability harness

OpenDQI's outputs are a stated product guarantee: **deterministic**
`summary.json` / `issues.csv` for a given input. This document covers
the test harness that locks that guarantee and the parser-robustness
contract.

## Golden snapshot regression tests

`crates/opendqi-cli/tests/golden.rs` runs the **real built `opendqi`
binary** over the synthetic `examples/` fixtures, once per
report-producing command family (EMIR & SFTR `scan`, `tr-state-scan`,
`tr-activity-scan`, `reconcile`, `book-reconcile`, `tr-audit`; EMIR
`feedback`, `recon-stats`, `mar-scan`, `msr-scan`), and asserts the
produced `summary.json` + issues CSV byte-for-byte against committed
goldens under `crates/opendqi-cli/tests/golden/`.

Two non-deterministic axes are normalized before comparison:

- **Absolute paths** — the canonical workspace root → `<WS>`, the
  system temp dir → `<TMP>`. Goldens contain **no machine-absolute
  paths** (portable across checkouts/CI).
- **Wall-clock timestamps** — `summary.json` `started_at` /
  `finished_at` → `1970-01-01T00:00:00Z`.

`report.html` is intentionally **not** snapshotted (minijinja +
timestamps — not stably comparable).

### Updating goldens after an intentional change

```bash
UPDATE_GOLDEN=1 cargo test -p opendqi-cli --test golden
git diff crates/opendqi-cli/tests/golden/   # review every changed line
```

A golden diff with no intended behavior change is a **regression** —
investigate before regenerating. The harness is dependency-free
(std only); no `insta`/`proptest`.

## Parser robustness contract

Every public parser entry point (`opendqi-xml` `read_*`, `opendqi-io`
CSV / zip / gzip ingestion) must, for **any** byte input —
well-formed, malformed, truncated, hostile — either return
`Ok(outcome)` (surfacing format problems as `DqIssue`s) or a clean
`Err`. It must **never panic, OOM, or hang**. Archive ingestion is
zip-slip / path-traversal hardened. This contract is exercised by an
adversarial corpus + deterministic byte-mutation suite under
`crates/opendqi-xml/tests/` (added incrementally; this section is the
authoritative statement of the guarantee).
